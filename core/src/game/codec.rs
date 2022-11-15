use blaze_pk::{codec::Codec, packet, packet::Packet, tag::ValueType, tagging::*};
use database::players;
use log::debug;

use crate::blaze::{
    components::{Components, GameManager},
    session::{SessionArc, SessionData},
};

use super::game::{AttrMap, Game};

packet! {
    // Packet for game state changes
    struct StateChange {
        // The id of the game the state has changed for
        GID id: u32,
        // The new state value
        GSTA state: u16
    }
}

packet! {
    // Packet for game setting changes
    struct SettingChange {
        // The new setting value
        ATTR setting: u16,
        // The id of the game the setting has changed for
        GID id: u32,
    }
}

/// Packet for game attribute changes
pub struct AttributesChange<'a> {
    /// The id of the game the attributes have changed for
    pub id: u32,
    /// Borrowed game attributes map
    pub attributes: &'a AttrMap,
}

impl Codec for AttributesChange<'_> {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_value(output, "ATTR", self.attributes);
        tag_u32(output, "GID", self.id);
    }
}

/// Encodes the provided players data to the provided output
///
/// `session_data` The session data to encode
/// `player`       The player attached to the session
/// `game_id`      The game the session is in
/// `slot`         The slot in the game the session is in
/// `output`       The output to encode to
fn encode_player_data(
    session_data: &SessionData,
    player: &players::Model,
    game_id: u32,
    slot: usize,
    output: &mut Vec<u8>,
) {
    tag_empty_blob(output, "BLOB");
    tag_u8(output, "EXID", 0);
    tag_u32(output, "GID", game_id);
    tag_u32(output, "LOC", 0x64654445);
    tag_str(output, "NAME", &player.display_name);
    let player_id = session_data.id_safe();
    tag_u32(output, "PID", player_id);
    tag_value(output, "PNET", &session_data.net.get_groups());
    tag_usize(output, "SID", slot);
    tag_u8(output, "SLOT", 0);
    tag_u8(output, "STAT", session_data.state);
    tag_u16(output, "TIDX", 0xffff);
    tag_u8(output, "TIME", 0);
    tag_triple(output, "UGID", &(0, 0, 0));
    tag_u32(output, "UID", player_id);
    tag_group_end(output);
}

pub struct PlayerJoining<'a> {
    /// The player ID of the joining player
    pub id: u32,
    /// The slot the player is joining into
    pub slot: usize,
    /// The session of the player that is joining
    pub session: &'a SessionData,
}

impl Codec for PlayerJoining<'_> {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_u32(output, "GID", self.id);

        if let Some(player) = self.session.player.as_ref() {
            tag_group_start(output, "PDAT");
            encode_player_data(self.session, player, self.id, self.slot, output);
        }
    }
}

pub async fn create_game_setup(game: &Game, host: bool, session: &SessionArc) -> Packet {
    let mut output = Vec::new();
    encode_game_setup(game, host, session, &mut output).await;
    Packet::notify_raw(Components::GameManager(GameManager::GameSetup), output)
}

async fn encode_game_setup(game: &Game, host: bool, session: &SessionArc, output: &mut Vec<u8>) {
    let players = &*game.players.read().await;
    let players_count = players.len();

    let mut player_data = Vec::new();
    let mut player_ids = Vec::with_capacity(players_count);

    let mut slot = 0;
    for session in players {
        let session_data = &*session.data.read().await;
        if let Some(player) = session_data.player.as_ref() {
            player_ids.push(player.id);
            encode_player_data(session_data, player, game.id, slot, &mut player_data);
        }
        slot += 1;
    }

    {
        let Some(host_session) = players.first() else {
            debug!("Unable to create setup notify when host is missing");
            return;
        };

        let host_data = host_session.data.read().await;
        let (host_id, game_name) = {
            if let Some(player) = host_data.player.as_ref() {
                (player.id, player.display_name.clone())
            } else {
                debug!("Unable to create setup notify when host player is missing");
                return;
            }
        };

        let game_data = game.data.read().await;
        tag_group_start(output, "GAME");
        tag_list(output, "ADMN", player_ids);
        tag_value(output, "ATTR", &game_data.attributes);
        tag_list(output, "CAP", vec![0x4, 0x0]);
        tag_u32(output, "GID", game.id);
        tag_str(output, "GNAM", &game_name);
        tag_u64(output, "GPVH", 0x5a4f2b378b715c6);
        tag_u16(output, "GSET", game_data.setting);
        tag_u64(output, "GSID", 0x4000000a76b645);
        tag_u16(output, "GSTA", game_data.state);
        drop(game_data);

        tag_empty_str(output, "GTYP");
        {
            tag_list_start(output, "HNET", ValueType::Group, 1);
            {
                output.push(2);
                host_data.net.groups.encode(output);
            }
        }

        tag_u32(output, "HSES", host_id);
        tag_u8(output, "IGNO", 0);
        tag_u8(output, "MCAP", 0x4);
        tag_value(output, "NQOS", &host_data.net.ext);
        tag_u8(output, "NRES", 0x0);
        tag_u8(output, "NTOP", 0x0);
        tag_empty_str(output, "PGID");
        tag_empty_blob(output, "PGSR");

        {
            tag_group_start(output, "PHST");
            tag_u32(output, "HPID", host_id);
            tag_u8(output, "HSLT", 0x0);
            tag_group_end(output);
        }

        tag_u8(output, "PRES", 0x1);
        tag_empty_str(output, "PSAS");
        tag_u8(output, "QCAP", 0x0);
        tag_u32(output, "SEED", 0x4cbc8585);
        tag_u8(output, "TCAP", 0x0);

        {
            tag_group_start(output, "THST");
            tag_u32(output, "HPID", host_id);
            tag_u8(output, "HSLT", 0x0);
            tag_group_end(output);
        }

        tag_str(output, "UUID", "286a2373-3e6e-46b9-8294-3ef05e479503");
        tag_u8(output, "VOIP", 0x2);
        tag_str(output, "VSTR", "ME3-295976325-179181965240128");
        tag_empty_blob(output, "XNNC");
        tag_empty_blob(output, "XSES");
        tag_group_end(output);
    }

    tag_list_start(output, "PROS", ValueType::Group, players_count);
    output.extend_from_slice(&player_data);

    if !host {
        let session_data = session.data.read().await;
        tag_optional_start(output, "REAS", 0x3);
        {
            tag_group_start(output, "VALU");
            tag_u16(output, "FIT", 0x3f7a);
            tag_u16(output, "MAXF", 0x5460);
            tag_u32(output, "MSID", session.id);
            tag_u8(output, "RSLT", 0x2);
            tag_u32(output, "USID", session_data.id_safe());
            tag_group_end(output);
        }
    } else {
        tag_optional_start(output, "REAS", 0x0);
        {
            tag_group_start(output, "VALU");
            tag_u8(output, "DCTX", 0x0);
            tag_group_end(output);
        }
    }
}

packet! {
    struct PlayerStateChange {
        GID gid: u32,
        PID pid: u32,
        STAT state: u8,
    }
}

packet! {
    struct JoinComplete {
        GID game_id: u32,
        PID player_id: u32,
    }
}

packet! {
    struct AdminListChange {
        ALST player_id: u32,
        GID game_id: u32,
        OPER operation: AdminListOperation,
        UID host_id: u32,
    }
}

#[derive(Debug)]

pub enum AdminListOperation {
    Add,
    Remove,
}

impl Codec for AdminListOperation {
    fn encode(&self, output: &mut Vec<u8>) {
        match self {
            Self::Add => output.push(0),
            Self::Remove => output.push(1),
        }
    }
}

pub struct PlayerRemoved {
    pub game_id: u32,
    pub player_id: u32,
}

pub enum RemoveReason {
    // 0x6
    Generic,
    // 0x8
    Kick,
}

impl Codec for PlayerRemoved {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_u8(output, "CNTX", 0);
        tag_u32(output, "GID", self.game_id);
        tag_u32(output, "PID", self.player_id);
        tag_u8(output, "REAS", 0x6);
    }
}

packet! {
    struct FetchExtendedData {
        BUID id: u32,
    }
}

pub struct HostMigrateStart {
    pub game_id: u32,
    pub host_id: u32,
}

impl Codec for HostMigrateStart {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_u32(output, "GID", self.game_id);
        tag_u32(output, "HOST", self.host_id);
        tag_u8(output, "PMIG", 0x2);
        tag_u8(output, "SLOT", 0x0);
    }
}

packet! {
    struct HostMigrateFinished {
        GID game_id: u32,
    }
}
