use super::{AttrMap, Game, GamePlayer};
use crate::utils::{
    components::{Components, GameManager},
    types::{GameID, GameSlot, PlayerID},
};
use bitflags::bitflags;
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
#[repr(u8)]
pub enum GameState {
    NewState = 0x0,
    Initializing = 0x1,
    Virtual = 0x2,
    PreGame = 0x82,
    InGame = 0x83,
    PostGame = 0x4,
    Migrating = 0x5,
    Destructing = 0x6,
    Resetable = 0x7,
    ReplaySetup = 0x8,
}

impl GameState {
    /// Gets the state from the provided value
    ///
    /// `value` The value to get the state of
    pub fn from_value(value: u8) -> Self {
        match value {
            0x0 => Self::NewState,
            0x1 => Self::Initializing,
            0x2 => Self::Virtual,
            0x82 => Self::PreGame,
            0x83 => Self::InGame,
            0x4 => Self::PostGame,
            0x5 => Self::Migrating,
            0x6 => Self::Destructing,
            0x7 => Self::Resetable,
            0x8 => Self::ReplaySetup,
            // Default to initializing state
            _ => Self::Initializing,
        }
    }
}

impl Encodable for GameState {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.write_u8((*self) as u8);
    }
}

impl Decodable for GameState {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let value = reader.read_u8()?;
        Ok(Self::from_value(value))
    }
}

value_type!(GameState, TdfType::VarInt);

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct GameSettings: u16 {
        const NONE = 0;
        const OPEN_TO_BROWSING = 1;
        const OPEN_TO_MATCHMAKING = 2;
        const OPEN_TO_INVITES = 4;
        const OPEN_TO_JOIN_BY_PLAYER = 8;
        const HOST_MIGRATABLE = 0x10;
        const RANKED = 0x20;
        const ADMIN_ONLY_INVITES = 0x40;
        const ENFORCE_SINGLE_GROUP_JOIN = 0x80;
        const JOIN_IN_PROGRESS_SUPPORTED = 0x100;
        const ADMIN_INVITE_ONLY_IGNORE_ENTRY_CHECKS = 0x200;
        const IGNORE_ENTRY_CRITERIA_WITH_INVITE = 0x400;
        const ENABLE_PERSISTED_GAME_ID = 0x800;
        const ALLOW_SAME_TEAM_ID = 0x1000;
        const VIRTUALIZED = 0x2000;
        const SEND_ORPHANDED_GAME_REPORT_EVENT = 0x4000;
        const ALLOW_ANY_REPUTATION = 0x8000;
    }
}

impl Encodable for GameSettings {
    fn encode(&self, output: &mut TdfWriter) {
        output.write_u16(self.bits())
    }
}

impl Decodable for GameSettings {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        Ok(GameSettings::from_bits_retain(reader.read_u16()?))
    }
}

value_type!(GameSettings, TdfType::VarInt);

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PlayerState {
    /// Link between the mesh points is not connected
    Reserved = 0x0,
    Queued = 0x1,
    /// Link is being formed between two mesh points
    ActiveConnecting = 0x2,
    ActiveMigrating = 0x3,
    /// Link is connected between two mesh points
    ActiveConnected = 0x4,
    ActiveKickPending = 0x5,
}

impl PlayerState {
    /// Gets the mesh state from the provided value
    ///
    /// `value` The value of the mesh state
    pub fn from_value(value: u8) -> Self {
        match value {
            0x0 => Self::Reserved,
            0x1 => Self::Queued,
            0x2 => Self::ActiveConnecting,
            0x3 => Self::ActiveMigrating,
            0x4 => Self::ActiveConnected,
            0x5 => Self::ActiveKickPending,
            _ => Self::Reserved,
        }
    }
}

impl Encodable for PlayerState {
    fn encode(&self, output: &mut TdfWriter) {
        output.write_u8((*self) as u8)
    }
}

impl Decodable for PlayerState {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        Ok(PlayerState::from_value(reader.read_u8()?))
    }
}

value_type!(PlayerState, TdfType::VarInt);

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
    pub setting: GameSettings,
    /// The ID of the game
    pub id: GameID,
}

impl Encodable for SettingChange {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u16(b"ATTR", self.setting.bits());
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

pub enum GameSetupContext {
    /// Context without additional data
    Dataless(DatalessContext),
    /// Context added from matchmaking
    Matchmaking(u32),
}

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum DatalessContext {
    /// Session created the game
    CreateGameSetup = 0x0,
    /// Session joined by ID
    JoinGameSetup = 0x1,
    // IndirectJoinGameFromQueueSetup = 0x2,
    // IndirectJoinGameFromReservationContext = 0x3,
    // HostInjectionSetupContext = 0x4,
}

pub struct GameDetails<'a> {
    pub game: &'a Game,
    pub context: GameSetupContext,
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
            writer.tag_u16(b"GSET", game.setting.bits());
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

        match &self.context {
            GameSetupContext::Dataless(context) => {
                writer.tag_union_start(b"REAS", 0x0);
                writer.group(b"VALU", |writer| {
                    writer.tag_u8(b"DCTX", (*context) as u8);
                });
            }
            GameSetupContext::Matchmaking(id) => {
                writer.tag_union_start(b"REAS", 0x3);
                writer.group(b"VALU", |writer| {
                    const FIT: u16 = 21600;

                    writer.tag_u16(b"FIT", FIT);
                    writer.tag_u16(b"MAXF", FIT);
                    writer.tag_u32(b"MSID", *id);
                    // TODO: Matchmaking result
                    // SUCCESS_CREATED_GAME = 0
                    // SUCCESS_JOINED_NEW_GAME = 1
                    // SUCCESS_JOINED_EXISTING_GAME = 2
                    // SESSION_TIMED_OUT = 3
                    // SESSION_CANCELED = 4
                    // SESSION_TERMINATED = 5
                    // SESSION_ERROR_GAME_SETUP_FAILED = 6
                    writer.tag_u8(b"RSLT", 0x2);
                    writer.tag_u32(b"USID", *id);
                });
            }
        }
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
        writer.tag_u16(b"GSET", game.setting.bits());
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

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum RemoveReason {
    /// Hit timeout while joining
    JoinTimeout = 0x0,
    /// Player lost PTP conneciton
    PlayerConnectionLost = 0x1,
    /// Player lost connection with the Pocket Relay server
    ServerConnectionLost = 0x2,
    /// Game migration failed
    MigrationFailed = 0x3,
    GameDestroyed = 0x4,
    GameEnded = 0x5,
    /// Generic player left the game reason
    PlayerLeft = 0x6,
    GroupLeft = 0x7,
    /// Player kicked
    PlayerKicked = 0x8,
    /// Player kicked and banned
    PlayerKickedWithBan = 0x9,
    /// Failed to join from the queue
    PlayerJoinFromQueueFailed = 0xA,
    PlayerReservationTimeout = 0xB,
    HostEjected = 0xC,
}

impl RemoveReason {
    pub fn from_value(value: u8) -> Self {
        match value {
            0x0 => Self::JoinTimeout,
            0x1 => Self::PlayerConnectionLost,
            0x2 => Self::ServerConnectionLost,
            0x3 => Self::MigrationFailed,
            0x4 => Self::GameDestroyed,
            0x5 => Self::GameEnded,
            0x6 => Self::PlayerLeft,
            0x7 => Self::GroupLeft,
            0x8 => Self::PlayerKicked,
            0x9 => Self::PlayerKickedWithBan,
            0xA => Self::PlayerJoinFromQueueFailed,
            0xB => Self::PlayerReservationTimeout,
            0xC => Self::HostEjected,
            // Default to generic reason for unknown
            _ => Self::PlayerLeft,
        }
    }
}

impl Encodable for RemoveReason {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.write_u8((*self) as u8);
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
pub struct AsyncMatchmakingStatus {
    pub player_id: PlayerID,
}

impl Encodable for AsyncMatchmakingStatus {
    fn encode(&self, writer: &mut TdfWriter) {
        {
            writer.tag_list_start(b"ASIL", TdfType::Group, 1);
            // Create game status
            writer.group(b"CGS", |writer| {
                // Evaluate status
                // PlayerCountSufficient = 1,
                // AcceptableHostFound = 2,
                // TeamSizesSufficient = 4
                writer.tag_u8(b"EVST", 2 | 4);
                // Number of matchmaking sessions
                writer.tag_u8(b"MMSN", 1);
                // Number of matched players
                writer.tag_u8(b"NOMP", 0);
            });

            // Custom async status
            writer.group(b"CUST", |_| {});

            // DNF rule status
            writer.group(b"DNFS", |writer| {
                // My DNF value
                writer.tag_zero(b"MDNF");
                // Max DNF value
                writer.tag_zero(b"XDNF");
            });

            // Find game status
            writer.group(b"FGS", |writer| {
                // Number of games
                writer.tag_zero(b"GNUM");
            });

            // Geo location rule status
            writer.group(b"GEOS", |writer| {
                // Max distance
                writer.tag_zero(b"DIST");
            });

            // Generic rule status dictionary (TODO: RULES HERE)
            writer.tag_map_start(b"GRDA", TdfType::String, TdfType::Group, 0);

            // Game size rule status
            writer.group(b"GSRD", |writer| {
                // Max player count accepted
                writer.tag_u8(b"PMAX", 4);
                // Min player count accepted
                writer.tag_u8(b"PMIN", 2);
            });

            // Host balance rule status
            writer.group(b"HBRD", |writer| {
                // Host balance values
                // HOSTS_STRICTLY_BALANCED = 0,
                // HOSTS_BALANCED = 1,
                // HOSTS_UNBALANCED = 2,

                writer.tag_u8(b"BVAL", 1);
            });

            // Host viability rule status
            writer.group(b"HVRD", |writer| {
                // Host viability values
                // CONNECTION_ASSURED = 0,
                // CONNECTION_LIKELY = 1,
                // CONNECTION_FEASIBLE = 2,
                // CONNECTION_UNLIKELY = 3,

                writer.tag_zero(b"VVAL");
            });

            // Ping site rule status
            writer.group(b"PSRS", |_| {});

            // Rank rule status
            writer.group(b"RRDA", |writer| {
                // Matched rank flags
                writer.tag_zero(b"RVAL");
            });

            // Team size rule status
            writer.group(b"TSRS", |writer| {
                // Max team size accepted
                writer.tag_zero(b"TMAX");
                // Min team size accepted
                writer.tag_zero(b"TMIN");
            });

            // UED rule status
            writer.tag_map_start(b"GRDA", TdfType::String, TdfType::Group, 0);
            // Virtual game rule status
            writer.group(b"VGRS", |writer| writer.tag_zero(b"VVAL"));
            writer.tag_group_end();
        }

        writer.tag_u32(b"MSID", self.player_id);
        writer.tag_u32(b"USID", self.player_id);
    }
}
