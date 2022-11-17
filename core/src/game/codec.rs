use blaze_pk::{
    codec::{Codec, CodecResult, Reader},
    packet,
    packet::Packet,
    tag::ValueType,
    tagging::*,
};

use serde::Serialize;
use utils::types::{GameID, GameSlot, PlayerID};

use crate::blaze::components::{Components, GameManager};

use super::{
    game::{AttrMap, Game},
    player::GamePlayer,
};

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
pub enum GameSetupType {
    Created,
    Joined,
}

impl GameSetupType {
    pub fn value(&self) -> u8 {
        match self {
            Self::Created => 0x0,
            Self::Joined => 0x3,
        }
    }
}

impl Into<u8> for GameSetupType {
    fn into(self) -> u8 {
        self.value()
    }
}

/// Values: 285 (0x11d), 287 (0x11f), 1311 (0x51f)
#[allow(unused)]
pub enum GameSetting {}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    /// Initial game state
    Init,
    /// In Lobby
    InGame,
    /// Game starting / Active
    InGameStarting,
    /// Game is finished
    GameFinished,
    /// Host is migrating
    HostMigration,
    Unknown(u8),
}

impl GameState {
    pub fn value(&self) -> u8 {
        match self {
            Self::Init => 0x1,
            Self::InGame => 0x82,
            Self::InGameStarting => 0x83,
            Self::GameFinished => 0x4,
            Self::HostMigration => 0x5,
            Self::Unknown(value) => *value,
        }
    }

    pub fn from_value(value: u8) -> Self {
        match value {
            0x1 => Self::Init,
            0x82 => Self::InGame,
            0x83 => Self::InGameStarting,
            0x4 => Self::GameFinished,
            0x5 => Self::HostMigration,
            value => Self::Unknown(value),
        }
    }
}

impl Codec for GameState {
    fn encode(&self, output: &mut Vec<u8>) {
        let value = self.value();
        value.encode(output);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let value = u8::decode(reader)?;
        Ok(Self::from_value(value))
    }

    fn value_type() -> ValueType {
        ValueType::VarInt
    }
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Disconnected,
    Connecting,
    Connected,
}

impl PlayerState {
    pub fn value(&self) -> u8 {
        match self {
            Self::Disconnected => 0x0,
            Self::Connecting => 0x2,
            Self::Connected => 0x4,
        }
    }
}

impl Codec for PlayerState {
    fn encode(&self, output: &mut Vec<u8>) {
        let value = self.value();
        value.encode(output);
    }

    fn value_type() -> ValueType {
        ValueType::VarInt
    }
}

packet! {
    // Packet for game state changes
    struct StateChange {
        // The id of the game the state has changed for
        GID id: GameID,
        // The new state value
        GSTA state: GameState
    }
}

packet! {
    // Packet for game setting changes
    struct SettingChange {
        // The new setting value
        ATTR setting: u16,
        // The id of the game the setting has changed for
        GID id: GameID,
    }
}

/// Packet for game attribute changes
pub struct AttributesChange<'a> {
    /// The id of the game the attributes have changed for
    pub id: GameID,
    /// Borrowed game attributes map
    pub attributes: &'a AttrMap,
}

impl Codec for AttributesChange<'_> {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_value(output, "ATTR", self.attributes);
        tag_u32(output, "GID", self.id);
    }
}

pub struct PlayerJoining<'a> {
    /// The slot the player is joining into
    pub slot: GameSlot,
    /// The player that is joining
    pub player: &'a GamePlayer,
}

impl Codec for PlayerJoining<'_> {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_u32(output, "GID", self.player.game_id);

        tag_group_start(output, "PDAT");
        self.player.encode(self.slot, output);
    }
}

pub async fn create_game_setup(game: &Game, player: &GamePlayer) -> Packet {
    let mut output = Vec::new();
    encode_game_setup(game, player, &mut output).await;
    Packet::notify_raw(Components::GameManager(GameManager::GameSetup), output)
}

async fn encode_game_setup(game: &Game, player: &GamePlayer, output: &mut Vec<u8>) {
    let players = &*game.players.read().await;
    let mut player_ids = players
        .iter()
        .map(|value| value.player_id)
        .collect::<Vec<_>>();
    player_ids.push(player.player_id);

    {
        let host_player = players.first().unwrap_or(player);
        let game_name = host_player.display_name.clone();
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
        tag_value(output, "GSTA", &game_data.state);
        drop(game_data);

        tag_empty_str(output, "GTYP");
        {
            tag_list_start(output, "HNET", ValueType::Group, 1);
            {
                output.push(2);
                host_player.net.groups.encode(output);
            }
        }

        tag_u32(output, "HSES", host_player.session_id);
        tag_u8(output, "IGNO", 0);
        tag_u8(output, "MCAP", 0x4);
        tag_value(output, "NQOS", &host_player.net.qos);
        tag_u8(output, "NRES", 0x0);
        tag_u8(output, "NTOP", 0x0);
        tag_empty_str(output, "PGID");
        tag_empty_blob(output, "PGSR");

        {
            tag_group_start(output, "PHST");
            tag_u32(output, "HPID", host_player.player_id);
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
            tag_u32(output, "HPID", host_player.player_id);
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

    tag_list_start(output, "PROS", ValueType::Group, players.len() + 1);
    let mut slot = 0;
    for session in players {
        session.encode(slot, output);
        slot += 1;
    }
    player.encode(slot, output);
    // If we are not the first player in the game aka the host
    if slot != 0 {
        tag_union_start(output, "REAS", GameSetupType::Joined.into());
        {
            tag_group_start(output, "VALU");
            tag_u16(output, "FIT", 0x3f7a);
            tag_u16(output, "MAXF", 0x5460);
            tag_u32(output, "MSID", player.session_id);
            tag_u8(output, "RSLT", 0x2);
            tag_u32(output, "USID", player.session_id);
            tag_group_end(output);
        }
    } else {
        tag_union_start(output, "REAS", GameSetupType::Created.into());
        {
            tag_group_start(output, "VALU");
            tag_u8(output, "DCTX", 0x0);
            tag_group_end(output);
        }
    }
}

packet! {
    struct PlayerStateChange {
        GID gid: GameID,
        PID pid: PlayerID,
        STAT state: PlayerState,
    }
}

packet! {
    struct JoinComplete {
        GID game_id: GameID,
        PID player_id: PlayerID,
    }
}

packet! {
    struct AdminListChange {
        ALST player_id: PlayerID,
        GID game_id: GameID,
        OPER operation: AdminListOperation,
        UID host_id: PlayerID,
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
    pub game_id: GameID,
    pub player_id: PlayerID,
}

pub enum RemoveReason {
    // 0x0
    JoinTimeout,
    /// 0x1
    ConnectionLost,
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
        BUID player_id: PlayerID,
    }
}

pub struct HostMigrateStart {
    pub game_id: GameID,
    pub host_id: PlayerID,
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
        GID game_id: GameID,
    }
}
