use crate::blaze::components::{Components, GameManager};
use crate::blaze::errors::{GameError, GameResult};
use crate::blaze::{SessionArc, SessionData};
use crate::game::Game;
use blaze_pk::{
    packet, tag_empty_blob, tag_empty_str, tag_group_end, tag_group_start, tag_list,
    tag_list_start, tag_optional_start, tag_str, tag_triple, tag_u16, tag_u32, tag_u64, tag_u8,
    tag_usize, tag_value, Codec, OpaquePacket, Packets, TdfMap, ValueType,
};

pub struct NotifyPlayerJoining<'a> {
    /// ID of the game that the player is joining
    pub id: u32,
    /// The session data of the player that is joining
    pub session: &'a SessionData,
}

impl Codec for NotifyPlayerJoining<'_> {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_u32(output, "GID", self.id);
        tag_group_start(output, "PDAT");
        encode_player_data(self.session, output);
    }
}

pub fn encode_player_data(session: &SessionData, output: &mut Vec<u8>) {
    let Some(player) = session.player else {return;};
    let Some(game) = session.game else { return; };

    tag_empty_blob(output, "BLOB");
    tag_u8(output, "EXID", 0);
    tag_u32(output, "GID", game.game.id);
    tag_u32(output, "LOC", session.location);
    tag_str(output, "NAME", &player.display_name);
    let player_id = session.player_id_safe();
    tag_u32(output, "PID", player_id);
    tag_value(output, "PNET", &session.net.get_groups());
    tag_usize(output, "SID", game.slot);
    tag_u8(output, "SLOT", 0);
    tag_u8(output, "STAT", session.state);
    tag_u16(output, "TIDX", 0xffff);
    tag_u8(output, "TIME", 0);
    tag_triple(output, "UGID", &(0, 0, 0));
    tag_u32(output, "UID", player_id);
    tag_group_end(output);
}

pub async fn notify_game_setup(game: &Game, session: &SessionArc) -> GameResult<OpaquePacket> {
    let mut output = Vec::new();
    encode_notify_game_setup(game, session, &mut output).await?;
    Ok(Packets::notify_raw(
        Components::GameManager(GameManager::GameSetup),
        output,
    ))
}

//noinspection SpellCheckingInspection
async fn encode_notify_game_setup(
    game: &Game,
    session: &SessionArc,
    output: &mut Vec<u8>,
) -> GameResult<()> {
    let session_data = session.data.read().await;
    let mut player_data = Vec::new();
    let mut player_ids = Vec::new();

    let players = &*game.players.read().await;
    let player_count = players.len();

    for player in players {
        let session_data = player.data.read().await;
        player_ids.push(session_data.player_id_safe());
        encode_player_data(&session_data, &mut player_data);
    }

    let host = players.get(0).ok_or(GameError::MissingHost)?;

    let host_data = host.data.read().await;
    let host_id = host_data.player_id_safe();

    {
        let game_data = game.data.read().await;
        tag_group_start(output, "GAME");
        tag_list(output, "ADMN", player_ids);
        tag_value(output, "ATTR", &game_data.attributes);
        tag_list(output, "CAP", vec![0x4, 0x0]);
        tag_u32(output, "GID", game.id);
        tag_str(output, "GNAM", &game.name);
        tag_u64(output, "GPVH", Game::GPVH);
        tag_u16(output, "GSET", game_data.setting);
        tag_u64(output, "GSID", Game::GSID);
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

    tag_list_start(output, "PROS", ValueType::Group, player_count);
    output.extend_from_slice(&player_data);

    let game_slot = session_data
        .game
        .as_ref()
        .map(|value| value.slot)
        .unwrap_or(0);

    if game_slot != 0 {
        tag_optional_start(output, "REAS", 0x3);
        {
            tag_group_start(output, "VALU");
            tag_u16(output, "FIT", 0x3f7a);
            tag_u16(output, "MAXF", 0x5460);
            tag_u32(output, "MSID", session.id);
            tag_u8(output, "RSLT", 0x2);
            tag_u32(output, "USID", session_data.player_id_safe());
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
    Ok(())
}

packet! {
    struct NotifyStateChange {
        GID id: u32,
        GSTA state: u16,
    }
}

packet! {
    struct NotifySettingChange {
        ATTR setting: u16,
        GID id: u32,
    }
}

pub struct NotifyAttribsChange<'a> {
    pub attributes: &'a TdfMap<String, String>,
    pub id: u32,
}

impl Codec for NotifyAttribsChange<'_> {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_value(output, "ATTR", self.attributes);
        tag_u32(output, "GID", self.id)
    }
}

pub struct NotifyPlayerRemoved {
    pub id: u32,
    pub pid: u32,
}

impl Codec for NotifyPlayerRemoved {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_u8(output, "CNTX", 0);
        tag_u32(output, "GID", self.id);
        tag_u32(output, "PID", self.pid);
        tag_u8(output, "REAS", 0x6);
    }
}

packet! {
    struct FetchExtendedData {
        BUID id: u32
    }
}
