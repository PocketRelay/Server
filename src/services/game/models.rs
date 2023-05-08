use super::{AttrMap, Game, GamePlayer};
use crate::utils::{
    components::{Components, GameManager},
    types::{GameID, GameSlot, PlayerID, SessionID},
};
use blaze_pk::{
    codec::{Decodable, Encodable},
    error::DecodeResult,
    packet::Packet,
    reader::TdfReader,
    tag::TdfType,
    value_type,
    writer::TdfWriter,
};
use serde::Serialize;

/// Different states the game can be in
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
    /// Unknown state
    Unknown(u8),
}

impl GameState {
    /// Gets the int value of the state
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

    /// Gets the state from the provided value
    ///
    /// `value` The value to get the state of
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

impl Encodable for GameState {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.write_u8(self.value());
    }
}

impl Decodable for GameState {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let value = reader.read_u8()?;
        Ok(Self::from_value(value))
    }
}

value_type!(GameState, TdfType::VarInt);

/// Mesh connection state type
#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
pub enum MeshState {
    /// Link between the mesh points is not connected
    Disconnected,
    /// Link is being formed between two mesh points
    Connecting,
    /// Link is connected between two mesh points
    Connected,
    /// Unknown mesh link state
    Unknown(u8),
}

impl MeshState {
    /// Converts the mesh state into its byte value
    pub fn value(&self) -> u8 {
        match self {
            Self::Disconnected => 0x0,
            Self::Connecting => 0x2,
            Self::Connected => 0x4,
            Self::Unknown(value) => *value,
        }
    }

    /// Gets the mesh state from the provided value
    ///
    /// `value` The value of the mesh state
    pub fn from_value(value: u8) -> Self {
        match value {
            0x0 => Self::Disconnected,
            0x2 => Self::Connecting,
            0x4 => Self::Connected,
            value => Self::Unknown(value),
        }
    }
}

impl Encodable for MeshState {
    fn encode(&self, output: &mut TdfWriter) {
        output.write_u8(self.value())
    }
}

impl Decodable for MeshState {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        Ok(MeshState::from_value(reader.read_u8()?))
    }
}

value_type!(MeshState, TdfType::VarInt);

/// Message for a game state changing
pub struct StateChange {
    /// The ID of the game
    pub id: GameID,
    /// The game state
    pub state: GameState,
}

impl Encodable for StateChange {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u32(b"GID", self.id);
        writer.tag_value(b"GSTA", &self.state);
    }
}

/// Message for a game setting changing
pub struct SettingChange {
    /// The game setting
    pub setting: u16,
    /// The ID of the game
    pub id: GameID,
}

impl Encodable for SettingChange {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u16(b"ATTR", self.setting);
        writer.tag_u32(b"GID", self.id);
    }
}

/// Packet for game attribute changes
pub struct AttributesChange<'a> {
    /// The id of the game the attributes have changed for
    pub id: GameID,
    /// Borrowed game attributes map
    pub attributes: &'a AttrMap,
}

impl Encodable for AttributesChange<'_> {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_value(b"ATTR", self.attributes);
        writer.tag_u32(b"GID", self.id);
    }
}

/// Message for a player joining notification
pub struct PlayerJoining<'a> {
    /// The ID of the game
    pub game_id: GameID,
    /// The slot the player is joining into
    pub slot: GameSlot,
    /// The player that is joining
    pub player: &'a GamePlayer,
}

impl Encodable for PlayerJoining<'_> {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u32(b"GID", self.game_id);

        writer.tag_group(b"PDAT");
        self.player.encode(self.game_id, self.slot, writer);
    }
}

fn write_admin_list(writer: &mut TdfWriter, game: &Game) {
    writer.tag_list_start(b"ADMN", TdfType::VarInt, game.players.len());
    for player in &game.players {
        writer.write_u32(player.player.id);
    }
}

const VSTR: &str = "ME3-295976325-179181965240128";

pub struct GameDetails<'a> {
    pub game: &'a Game,
    pub msid: Option<SessionID>,
}

impl Encodable for GameDetails<'_> {
    fn encode(&self, writer: &mut TdfWriter) {
        let game = self.game;
        let host_player = match game.players.first() {
            Some(value) => value,
            None => return,
        };

        // Game details
        writer.group(b"GAME", |writer| {
            write_admin_list(writer, game);
            writer.tag_value(b"ATTR", &game.attributes);
            {
                writer.tag_list_start(b"CAP", TdfType::VarInt, 2);
                writer.write_u8(4);
                writer.write_u8(0);
            }

            writer.tag_u32(b"GID", game.id);
            writer.tag_str(b"GNAM", &host_player.player.display_name);

            writer.tag_u64(b"GPVH", 0x5a4f2b378b715c6);
            writer.tag_u16(b"GSET", game.setting);
            writer.tag_u64(b"GSID", 0x4000000a76b645);
            writer.tag_value(b"GSTA", &game.state);

            writer.tag_str_empty(b"GTYP");
            {
                writer.tag_list_start(b"HNET", TdfType::Group, 1);
                writer.write_byte(2);
                if let Some(groups) = &host_player.net.groups {
                    groups.encode(writer);
                }
            }

            writer.tag_u32(b"HSES", host_player.player.id);
            writer.tag_zero(b"IGNO");
            writer.tag_u8(b"MCAP", 4);
            writer.tag_value(b"NQOS", &host_player.net.qos);
            writer.tag_zero(b"NRES");
            writer.tag_zero(b"NTOP");
            writer.tag_str_empty(b"PGID");
            writer.tag_empty_blob(b"PGSR");

            writer.group(b"PHST", |writer| {
                writer.tag_u32(b"HPID", host_player.player.id);
                writer.tag_zero(b"HSLT");
            });

            writer.tag_u8(b"PRES", 0x1);
            writer.tag_str_empty(b"PSAS");
            writer.tag_u8(b"QCAP", 0x0);
            writer.tag_u32(b"SEED", 0x4cbc8585);
            writer.tag_u8(b"TCAP", 0x0);

            writer.group(b"THST", |writer| {
                writer.tag_u32(b"HPID", host_player.player.id);
                writer.tag_u8(b"HSLT", 0x0);
            });

            writer.tag_str(b"UUID", "286a2373-3e6e-46b9-8294-3ef05e479503");
            writer.tag_u8(b"VOIP", 0x2);
            writer.tag_str(b"VSTR", VSTR);
            writer.tag_empty_blob(b"XNNC");
            writer.tag_empty_blob(b"XSES");
        });

        // Player list
        writer.tag_list_start(b"PROS", TdfType::Group, game.players.len());
        for (slot, player) in game.players.iter().enumerate() {
            player.encode(game.id, slot, writer);
        }

        // Join details
        let union_value = if self.msid.is_some() { 0x3 } else { 0x0 };
        writer.tag_union_start(b"REAS", union_value);
        writer.group(b"VALU", |writer| {
            if let Some(msid) = self.msid {
                writer.tag_u16(b"FIT", 0x3f7a);
                writer.tag_u16(b"MAXF", 0x5460);
                writer.tag_u32(b"MSID", msid);
                writer.tag_u8(b"RSLT", 0x2);
                writer.tag_u32(b"USID", msid);
            } else {
                writer.tag_u8(b"DCTX", 0x0);
            }
        });
    }
}

pub struct GetGameDetails<'a> {
    pub game: &'a Game,
}

impl Encodable for GetGameDetails<'_> {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_list_start(b"GDAT", TdfType::Group, 1);
        let game = self.game;
        let host_player = match game.players.first() {
            Some(value) => value,
            None => return,
        };

        write_admin_list(writer, game);
        writer.tag_value(b"ATTR", &game.attributes);
        {
            writer.tag_list_start(b"CAP", TdfType::VarInt, 2);
            writer.write_u8(4);
            writer.write_u8(0);
        }
        writer.tag_u32(b"GID", game.id);
        writer.tag_str(b"GNAM", &host_player.player.display_name);
        writer.tag_u16(b"GSET", game.setting);
        writer.tag_value(b"GSTA", &game.state);
        {
            writer.tag_list_start(b"HNET", TdfType::Group, 1);
            writer.write_byte(2);
            if let Some(groups) = &host_player.net.groups {
                groups.encode(writer);
            }
        }
        writer.tag_u32(b"HOST", host_player.player.id);
        writer.tag_zero(b"NTOP");

        {
            writer.tag_list_start(b"PCNT", TdfType::VarInt, 2);
            writer.write_u8(1);
            writer.write_u8(0);
        }

        writer.tag_u8(b"PRES", 0x2);
        writer.tag_str(b"PSAS", "ea-sjc");
        writer.tag_str_empty(b"PSID");
        writer.tag_zero(b"QCAP");
        writer.tag_zero(b"QCNT");
        writer.tag_zero(b"SID");
        writer.tag_zero(b"TCAP");
        writer.tag_u8(b"VOIP", 0x2);
        writer.tag_str(b"VSTR", VSTR);
        writer.tag_group_end();
    }
}

pub struct PlayerStateChange {
    pub gid: GameID,
    pub pid: PlayerID,
    pub state: MeshState,
}

impl Encodable for PlayerStateChange {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u32(b"GID", self.gid);
        writer.tag_u32(b"PID", self.pid);
        writer.tag_value(b"STAT", &self.state);
    }
}

pub struct JoinComplete {
    pub game_id: GameID,
    pub player_id: PlayerID,
}

impl Encodable for JoinComplete {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u32(b"GID", self.game_id);
        writer.tag_u32(b"PID", self.player_id);
    }
}

pub struct AdminListChange {
    pub player_id: PlayerID,
    pub game_id: GameID,
    pub operation: AdminListOperation,
    pub host_id: PlayerID,
}

/// Different operations that can be performed on
/// the admin list
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum AdminListOperation {
    Add = 0,
    Remove = 1,
}

impl Encodable for AdminListChange {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u32(b"ALST", self.player_id);
        writer.tag_u32(b"GID", self.game_id);
        writer.tag_u8(b"OPER", self.operation as u8);
        writer.tag_u32(b"UID", self.host_id);
    }
}

pub struct PlayerRemoved {
    pub game_id: GameID,
    pub player_id: PlayerID,
    pub reason: RemoveReason,
}

#[derive(Debug)]
#[repr(u8)]
pub enum RemoveReason {
    // 0x0
    JoinTimeout,
    /// 0x1
    ConnectionLost,
    // 0x6
    Generic,
    // 0x8
    Kick,
    // Unknown value
    Unknown(u8),
}

impl RemoveReason {
    pub fn from_value(value: u8) -> Self {
        match value {
            0 => Self::JoinTimeout,
            1 => Self::ConnectionLost,
            6 => Self::Generic,
            8 => Self::Kick,
            value => Self::Unknown(value),
        }
    }

    pub fn to_value(&self) -> u8 {
        match self {
            Self::JoinTimeout => 0,
            Self::ConnectionLost => 1,
            Self::Generic => 6,
            Self::Kick => 8,
            Self::Unknown(value) => *value,
        }
    }
}

impl Encodable for RemoveReason {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.write_u8(self.to_value());
    }
}

impl Decodable for RemoveReason {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let value: u8 = reader.read_u8()?;
        Ok(Self::from_value(value))
    }
}

value_type!(RemoveReason, TdfType::VarInt);

impl Encodable for PlayerRemoved {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u8(b"CNTX", 0);
        writer.tag_u32(b"GID", self.game_id);
        writer.tag_u32(b"PID", self.player_id);
        writer.tag_value(b"REAS", &self.reason);
    }
}

pub struct FetchExtendedData {
    pub player_id: PlayerID,
}

impl Encodable for FetchExtendedData {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u32(b"BUID", self.player_id);
    }
}

pub struct HostMigrateStart {
    pub game_id: GameID,
    pub host_id: PlayerID,
}

impl Encodable for HostMigrateStart {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u32(b"GID", self.game_id);
        writer.tag_u32(b"HOST", self.host_id);
        writer.tag_u8(b"PMIG", 0x2);
        writer.tag_u8(b"SLOT", 0x0);
    }
}

impl From<HostMigrateStart> for Packet {
    fn from(value: HostMigrateStart) -> Self {
        Packet::notify(
            Components::GameManager(GameManager::HostMigrationStart),
            value,
        )
    }
}

pub struct HostMigrateFinished {
    pub game_id: GameID,
}

impl Encodable for HostMigrateFinished {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u32(b"GID", self.game_id)
    }
}

impl From<HostMigrateFinished> for Packet {
    fn from(value: HostMigrateFinished) -> Self {
        Packet::notify(
            Components::GameManager(GameManager::HostMigrationFinished),
            value,
        )
    }
}

///
/// # Example
/// ```
/// Content: {
///  "ASIL": List<Group> [
///    {
///      "CGS": {
///        "EVST": 6,
///        "MMSN": 1,
///        "NOMP": 0,
///      },
///      "CUST": {
///      },
///      "DNFS": {
///        "MDNF": 0,
///        "XDNF": 0,
///      },
///      "FGS": {
///        "GNUM": 0,
///      },
///      "GEOS": {
///        "DIST": 0,
///      },
///      "GRDA": Map<String, Group> {
///        "ME3_gameDifficultyRule": {
///          "NAME": "ME3_gameDifficultyRule",
///          "VALU": List<String> ["difficulty0"],
///        }
///        "ME3_gameEnemyTypeRule": {
///          "NAME": "ME3_gameEnemyTypeRule",
///          "VALU": List<String> ["enemy0", "enemy1", "enemy2", "enemy3", "enemy4", "enemy5", "enemy6", "enemy7", "enemy8", "enemy9", "random", "abstain"],
///        }
///        "ME3_gameMapMatchRule": {
///          "NAME": "ME3_gameMapMatchRule",
///          "VALU": List<String> ["map0", "map1", "map2", "map3", "map4", "map5", "map6", "map7", "map8", "map9", "map10", "map11", "map12", "map13", "map14", "map15", "map16", "map17", "map18", "map19", "map20", "map21", "map22", "map23", "map24", "map25", "map26", "map27", "map28", "map29", "random", "abstain"],
///        }
///        "ME3_gameStateMatchRule": {
///          "NAME": "ME3_gameStateMatchRule",
///          "VALU": List<String> ["IN_LOBBY", "IN_LOBBY_LONGTIME", "IN_GAME_STARTING", "abstain"],
///        }
///        "ME3_rule_dlc2300": {
///          "NAME": "ME3_rule_dlc2300",
///          "VALU": List<String> ["required", "preferred"],
///        }
///        "ME3_rule_dlc2500": {
///          "NAME": "ME3_rule_dlc2500",
///          "VALU": List<String> ["required", "preferred"],
///        }
///        "ME3_rule_dlc2700": {
///          "NAME": "ME3_rule_dlc2700",
///          "VALU": List<String> ["required", "preferred"],
///        }
///        "ME3_rule_dlc3050": {
///          "NAME": "ME3_rule_dlc3050",
///          "VALU": List<String> ["required", "preferred"],
///        }
///        "ME3_rule_dlc3225": {
///          "NAME": "ME3_rule_dlc3225",
///          "VALU": List<String> ["required", "preferred"],
///        }
///      },
///      "GSRD": {
///        "PMAX": 4,
///        "PMIN": 2,
///      },
///      "HBRD": {
///        "BVAL": 1,
///      },
///      "HVRD": {
///        "VVAL": 0,
///      },
///      "PSRS": {
///      },
///      "RRDA": {
///        "RVAL": 0,
///      },
///      "TSRS": {
///        "TMAX": 0,
///        "TMIN": 0,
///      },
///      "UEDS": Map<String, Group> {
///        "ME3_characterSkill_Rule": {
///          "AMAX": 500,
///          "AMIN": 0,
///          "MUED": 500,
///          "NAME": "ME3_characterSkill_Rule",
///        }
///      },
///      "VGRS": {
///        "VVAL": 0,
///      },
///    }
///  ],
///  "MSID": 0x1,
///  "USID": 0x1,
///}
/// ```
#[allow(unused)]
pub struct AsyncMatchmakingStatus;
