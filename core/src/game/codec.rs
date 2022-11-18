use blaze_pk::{
    codec::{Codec, CodecResult, Reader},
    packet,
    tag::ValueType,
    tagging::*,
};

use serde::Serialize;
use utils::types::{GameID, GameSlot, PlayerID};

use super::{
    game::{AttrMap, GameData},
    player::GamePlayer,
};

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

pub fn encode_game_data(
    output: &mut Vec<u8>,
    id: GameID,
    players: &Vec<GamePlayer>,
    player: &GamePlayer,
    game_data: &GameData,
) {
    let mut player_ids = players
        .iter()
        .map(|value| value.player_id)
        .collect::<Vec<_>>();
    player_ids.push(player.player_id);
    let host_player = players.first().unwrap_or(player);

    tag_group_start(output, "GAME");
    let game_name = &host_player.display_name;
    tag_value(output, "ADMN", &player_ids);
    tag_value(output, "ATTR", &game_data.attributes);
    {
        tag_list_start(output, "CAP", ValueType::VarInt, 2);
        output.push(4);
        output.push(0);
    }
    tag_u32(output, "GID", id);
    tag_str(output, "GNAM", game_name);
    tag_u64(output, "GPVH", 0x5a4f2b378b715c6);
    tag_u16(output, "GSET", game_data.setting);
    tag_u64(output, "GSID", 0x4000000a76b645);
    tag_value(output, "GSTA", &game_data.state);

    tag_empty_str(output, "GTYP");
    {
        tag_list_start(output, "HNET", ValueType::Group, 1);
        output.push(2);
        host_player.net.groups.encode(output);
    }

    tag_u32(output, "HSES", host_player.session_id);
    tag_zero(output, "IGNO");
    tag_u8(output, "MCAP", 4);
    tag_value(output, "NQOS", &host_player.net.qos);
    tag_zero(output, "NRES");
    tag_zero(output, "NTOP");
    tag_empty_str(output, "PGID");
    tag_empty_blob(output, "PGSR");

    {
        tag_group_start(output, "PHST");
        tag_u32(output, "HPID", host_player.player_id);
        tag_zero(output, "HSLT");
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

pub fn encode_players_list(output: &mut Vec<u8>, players: &Vec<GamePlayer>, player: &GamePlayer) {
    tag_list_start(output, "PROS", ValueType::Group, players.len() + 1);
    let mut slot = 0;
    for player in players {
        player.encode(slot, output);
        slot += 1;
    }
    player.encode(slot, output);
}

pub struct GameDetails<'a> {
    pub id: GameID,
    pub players: &'a Vec<GamePlayer>,
    pub game_data: &'a GameData,
    pub player: &'a GamePlayer,
    pub ty: GameDetailsType,
}

impl Codec for GameDetails<'_> {
    fn encode(&self, output: &mut Vec<u8>) {
        encode_game_data(output, self.id, self.players, self.player, self.game_data);
        encode_players_list(output, self.players, self.player);
        let union_value = self.ty.value();
        tag_union_start(output, "REAS", union_value);
        tag_group_start(output, "VALU");
        match self.ty {
            GameDetailsType::Created => {
                tag_u8(output, "DCTX", 0x0);
            }
            GameDetailsType::Joined => {
                let session_id = self.player.session_id;
                tag_u16(output, "FIT", 0x3f7a);
                tag_u16(output, "MAXF", 0x5460);
                tag_u32(output, "MSID", session_id);
                tag_u8(output, "RSLT", 0x2);
                tag_u32(output, "USID", session_id);
            }
        }
        tag_group_end(output);
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
pub struct AsyncMatchmakingStatus;
