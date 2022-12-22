use super::{player::GamePlayer, AttrMap, Game};
use crate::utils::types::{GameID, GameSlot, PlayerID};
use blaze_pk::{
    codec::{Decodable, Encodable},
    error::DecodeResult,
    reader::TdfReader,
    tag::TdfType,
    value_type,
    writer::TdfWriter,
};
use serde::Serialize;

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
pub enum GameDetailsType {
    /// The player created the game the details are for
    Created,
    /// The player joined the game
    Joined,
}

impl GameDetailsType {
    pub fn value(&self) -> u8 {
        match self {
            Self::Created => 0x0,
            Self::Joined => 0x3,
        }
    }
}

/// Values: 285 (0x11d), 287 (0x11f), 1311 (0x51f)
#[allow(unused)]
pub enum GameSetting {}

// TODO: Game privacy

/// States that can be matched from the ME3gameState attribute
#[derive(Debug, PartialEq, Eq, Clone)]
#[allow(unused)]
pub enum GameStateAttr {
    /// Game has no state attribute
    None,
    /// IN_LOBBY: Players are waiting in lobby
    InLobby,
    /// IN_LOBBY_LONGTIME: Players have been waiting in lobby a long time
    InLobbyLongtime,
    /// IN_GAME_STARTING: Players in lobby all ready game almost started
    InGameStarting,
    /// IN_GAME_MIDGAME: The game is started and the players are playing
    InGameMidgame,
    /// IN_GAME_FINISHING: Game has finished and players returning to lobby
    InGameFinishing,
    /// MATCH_MAKING: Unknown how this state could be achieved but its present
    /// as a matchable value in async matchmaking status values
    ///
    /// Notice: Posibly joining two players together who are both searching for
    /// the same matchmaking game details
    MatchMaking,
    /// Unknown state not mentioned above
    Unknown(String),
}

#[allow(unused)]
impl GameStateAttr {
    const ATTR_KEY: &str = "ME3gameState";

    pub fn from_attrs(attrs: &AttrMap) -> Self {
        if let Some(value) = attrs.get(Self::ATTR_KEY) {
            match value as &str {
                "IN_LOBBY" => Self::InLobby,
                "IN_LOBBY_LONGTIME" => Self::InLobbyLongtime,
                "IN_GAME_STARTING" => Self::InGameStarting,
                "IN_GAME_MIDGAME" => Self::InGameMidgame,
                "IN_GAME_FINISHING" => Self::InGameFinishing,
                "MATCH_MAKING" => Self::MatchMaking,
                value => Self::Unknown(value.to_string()),
            }
        } else {
            Self::None
        }
    }
}

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

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[allow(unused)]
pub enum PlayerState {
    Disconnected,
    Connecting,
    Connected,
    Unknown(u8),
}

impl PlayerState {
    pub fn value(&self) -> u8 {
        match self {
            Self::Disconnected => 0x0,
            Self::Connecting => 0x2,
            Self::Connected => 0x4,
            Self::Unknown(value) => *value,
        }
    }

    pub fn from_value(value: u8) -> Self {
        match value {
            0x0 => Self::Disconnected,
            0x2 => Self::Connecting,
            0x4 => Self::Connected,
            value => Self::Unknown(value),
        }
    }
}

impl Encodable for PlayerState {
    fn encode(&self, output: &mut TdfWriter) {
        output.write_u8(self.value())
    }
}

impl Decodable for PlayerState {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        Ok(PlayerState::from_value(reader.read_u8()?))
    }
}

value_type!(PlayerState, TdfType::VarInt);

pub struct StateChange {
    pub id: GameID,
    pub state: GameState,
}

impl Encodable for StateChange {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u32(b"GID", self.id);
        writer.tag_value(b"GSTA", &self.state);
    }
}

pub struct SettingChange {
    pub setting: u16,
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

pub struct PlayerJoining<'a> {
    /// The slot the player is joining into
    pub slot: GameSlot,
    /// The player that is joining
    pub player: &'a GamePlayer,
}

impl Encodable for PlayerJoining<'_> {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u32(b"GID", self.player.game_id);

        writer.tag_group(b"PDAT");
        self.player.encode(self.slot, writer);
    }
}

pub fn encode_game_data(writer: &mut TdfWriter, game: &Game, player: &GamePlayer) {
    let mut player_ids = game
        .players
        .iter()
        .map(|value| value.player_id)
        .collect::<Vec<_>>();
    player_ids.push(player.player_id);
    let host_player = game.players.first().unwrap_or(player);

    writer.tag_group(b"GAME");

    let game_name = &host_player.display_name;

    writer.tag_value(b"ADMN", &player_ids);
    writer.tag_value(b"ATTR", &game.attributes);
    {
        writer.tag_list_start(b"CAP", TdfType::VarInt, 2);
        writer.write_u8(4);
        writer.write_u8(0);
    }

    writer.tag_u32(b"GID", game.id);
    writer.tag_str(b"GNAM", game_name);

    writer.tag_u64(b"GPVH", 0x5a4f2b378b715c6);
    writer.tag_u16(b"GSET", game.setting);
    writer.tag_u64(b"GSID", 0x4000000a76b645);
    writer.tag_value(b"GSTA", &game.state);

    writer.tag_str_empty(b"GTYP");
    {
        writer.tag_list_start(b"HNET", TdfType::Group, 1);
        writer.write_byte(2);
        host_player.net.groups.encode(writer);
    }

    writer.tag_u32(b"HSES", host_player.session_id);
    writer.tag_zero(b"IGNO");
    writer.tag_u8(b"MCAP", 4);
    writer.tag_value(b"NQOS", &host_player.net.qos);
    writer.tag_zero(b"NRES");
    writer.tag_zero(b"NTOP");
    writer.tag_str_empty(b"PGID");
    writer.tag_empty_blob(b"PGSR");

    {
        writer.tag_group(b"PHST");
        writer.tag_u32(b"HPID", host_player.player_id);
        writer.tag_zero(b"HSLT");
        writer.tag_group_end();
    }

    writer.tag_u8(b"PRES", 0x1);
    writer.tag_str_empty(b"PSAS");
    writer.tag_u8(b"QCAP", 0x0);
    writer.tag_u32(b"SEED", 0x4cbc8585);
    writer.tag_u8(b"TCAP", 0x0);

    {
        writer.tag_group(b"THST");
        writer.tag_u32(b"HPID", host_player.player_id);
        writer.tag_u8(b"HSLT", 0x0);
        writer.tag_group_end();
    }

    writer.tag_str(b"UUID", "286a2373-3e6e-46b9-8294-3ef05e479503");
    writer.tag_u8(b"VOIP", 0x2);
    writer.tag_str(b"VSTR", "ME3-295976325-179181965240128");
    writer.tag_empty_blob(b"XNNC");
    writer.tag_empty_blob(b"XSES");
    writer.tag_group_end();
}

pub fn encode_players_list(writer: &mut TdfWriter, players: &Vec<GamePlayer>, player: &GamePlayer) {
    writer.tag_list_start(b"PROS", TdfType::Group, players.len() + 1);
    let mut slot = 0;
    for player in players {
        player.encode(slot, writer);
        slot += 1;
    }
    player.encode(slot, writer);
}

pub struct GameDetails<'a> {
    pub game: &'a Game,
    pub player: &'a GamePlayer,
    pub ty: GameDetailsType,
}

impl Encodable for GameDetails<'_> {
    fn encode(&self, writer: &mut TdfWriter) {
        encode_game_data(writer, self.game, self.player);
        encode_players_list(writer, &self.game.players, self.player);
        let union_value = self.ty.value();
        writer.tag_union_start(b"REAS", union_value);
        writer.tag_group(b"VALU");
        match self.ty {
            GameDetailsType::Created => {
                writer.tag_u8(b"DCTX", 0x0);
            }
            GameDetailsType::Joined => {
                let session_id = self.player.session_id;
                writer.tag_u16(b"FIT", 0x3f7a);
                writer.tag_u16(b"MAXF", 0x5460);
                writer.tag_u32(b"MSID", session_id);
                writer.tag_u8(b"RSLT", 0x2);
                writer.tag_u32(b"USID", session_id);
            }
        }
        writer.tag_group_end();
    }
}

pub struct PlayerStateChange {
    pub gid: GameID,
    pub pid: PlayerID,
    pub state: PlayerState,
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

impl Encodable for AdminListChange {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u32(b"ALST", self.player_id);
        writer.tag_u32(b"GID", self.game_id);
        writer.tag_value(b"OPER", &self.operation);
        writer.tag_u32(b"UID", self.host_id);
    }
}

#[derive(Debug)]

pub enum AdminListOperation {
    Add,
    Remove,
}

impl Encodable for AdminListOperation {
    fn encode(&self, writer: &mut TdfWriter) {
        match self {
            Self::Add => writer.write_byte(0),
            Self::Remove => writer.write_byte(1),
        }
    }
}

value_type!(AdminListOperation, TdfType::VarInt);

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

pub struct HostMigrateFinished {
    pub game_id: GameID,
}

impl Encodable for HostMigrateFinished {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u32(b"GID", self.game_id)
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
