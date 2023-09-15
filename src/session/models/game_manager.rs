use bitflags::bitflags;
use serde::Serialize;
use tdf::{Blob, GroupSlice, TdfDeserialize, TdfDeserializeOwned, TdfSerialize, TdfType, TdfTyped};

use crate::{
    services::game::{rules::RuleSet, AttrMap, Game, GamePlayer},
    utils::types::{GameID, PlayerID, SessionID},
};

use super::NetworkAddress;

#[derive(Debug, Clone)]
#[repr(u16)]
#[allow(unused)]
pub enum GameManagerError {
    InvalidGameId = 0x2,
    GameFull = 0x4,
    PlayerNotFound = 0x65,
    AlreadyGameMember = 0x67,
    RemovePlayerFailed = 0x68,
    JoinPlayerFailed = 0x6c,
    AlreadyInQueue = 0x70,
    TeamFull = 0xff,
}

/// Structure of the request for creating new games contains the
/// initial game attributes and game setting
#[derive(TdfDeserialize)]
pub struct CreateGameRequest {
    /// The games initial attributes
    #[tdf(tag = "ATTR")]
    pub attributes: AttrMap,
    /// The games initial setting
    #[tdf(tag = "GSET", into = u16)]
    pub setting: GameSettings,
}

/// Structure for the response to game creation which contains
/// the ID of the created game
#[derive(TdfSerialize)]
pub struct CreateGameResponse {
    /// The game ID
    #[tdf(tag = "GID")]
    pub game_id: GameID,
}

/// Structure of request to remove player from a game
#[derive(TdfDeserialize)]
pub struct RemovePlayerRequest {
    /// The ID of the game to remove from
    #[tdf(tag = "GID")]
    pub game_id: GameID,
    /// The ID of the player to remove
    #[tdf(tag = "PID")]
    pub player_id: PlayerID,
    // The reason the player was removed
    #[tdf(tag = "REAS")]
    pub reason: RemoveReason,
}

#[derive(TdfDeserialize)]
pub struct SetAttributesRequest {
    /// The new game attributes
    #[tdf(tag = "ATTR")]
    pub attributes: AttrMap,
    /// The ID of the game to set the attributes for
    #[tdf(tag = "GID")]
    pub game_id: GameID,
}

#[derive(TdfDeserialize)]
pub struct SetStateRequest {
    #[tdf(tag = "GID")]
    pub game_id: GameID,
    #[tdf(tag = "GSTA")]
    pub state: GameState,
}

#[derive(TdfDeserialize)]
pub struct SetSettingRequest {
    #[tdf(tag = "GID")]
    pub game_id: GameID,
    #[tdf(tag = "GSET", into = u16)]
    pub setting: GameSettings,
}

/// Request to update the state of a mesh connection between
/// payers.
#[derive(TdfDeserialize)]
pub struct UpdateMeshRequest {
    #[tdf(tag = "GID")]
    pub game_id: GameID,
    #[tdf(tag = "TARG")]
    pub targets: Vec<MeshTarget>,
}

#[derive(TdfDeserialize, TdfTyped)]
#[tdf(group)]
pub struct MeshTarget {
    #[tdf(tag = "PID")]
    pub player_id: PlayerID,
    #[tdf(tag = "STAT")]
    pub state: PlayerState,
}

/// Structure of the request for starting matchmaking. Contains
/// the rule set that games must match in order to join
pub struct MatchmakingRequest {
    /// The matchmaking rule set
    pub rules: RuleSet,
}

impl TdfDeserializeOwned for MatchmakingRequest {
    fn deserialize_owned(r: &mut tdf::TdfDeserializer<'_>) -> tdf::DecodeResult<Self> {
        r.until_tag(b"CRIT", TdfType::Group)?;
        let rule_count: usize = r.until_list_typed(b"RLST", TdfType::Group)?;

        let mut rules: Vec<(String, String)> = Vec::with_capacity(rule_count);
        for _ in 0..rule_count {
            let name: String = r.tag(b"NAME")?;
            let values_count: usize = r.until_list_typed(b"VALU", TdfType::String)?;
            if values_count < 1 {
                continue;
            }
            let value: String = String::deserialize_owned(r)?;
            if values_count > 1 {
                for _ in 1..rule_count {
                    Blob::skip(r)?;
                }
            }
            GroupSlice::deserialize_content_skip(r)?;
            rules.push((name, value));
        }
        Ok(Self {
            rules: RuleSet::new(rules),
        })
    }
}

/// Structure of the matchmaking response. This just contains
/// what normally would be a unique matchmaking ID but in this case
/// its just the session ID.
#[derive(TdfSerialize)]
pub struct MatchmakingResponse {
    /// The current session ID
    #[tdf(tag = "MSID")]
    pub id: SessionID,
}

#[derive(TdfDeserialize)]
pub struct GetGameDataRequest {
    #[tdf(tag = "GLST")]
    pub game_list: Vec<GameID>,
}

#[derive(TdfDeserialize)]
pub struct JoinGameRequest {
    #[tdf(tag = "USER")]
    pub user: JoinGameRequestUser,
}

#[derive(TdfDeserialize, TdfTyped)]
#[tdf(group)]
pub struct JoinGameRequestUser {
    #[tdf(tag = "ID")]
    pub id: PlayerID,
}

#[derive(TdfSerialize)]
pub struct JoinGameResponse {
    #[tdf(tag = "GID")]
    pub game_id: GameID,
    #[tdf(tag = "JGS")]
    pub state: JoinGameState,
}

#[derive(TdfSerialize, TdfTyped, Copy, Clone)]
#[repr(u8)]
pub enum JoinGameState {
    JoinedGame = 0,
    // InQueue = 1,
    // GroupPartiallyJoined = 2,
}

#[derive(TdfSerialize)]
pub struct PlayerRemoved {
    #[tdf(tag = "CNTX")]
    pub cntx: u8,
    #[tdf(tag = "GID")]
    pub game_id: GameID,
    #[tdf(tag = "PID")]
    pub player_id: PlayerID,
    #[tdf(tag = "REAS")]
    pub reason: RemoveReason,
}

#[derive(Default, Debug, Clone, Copy, TdfSerialize, TdfDeserialize, TdfTyped)]
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
    #[tdf(default)]
    #[default]
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

#[derive(TdfSerialize)]
pub struct AdminListChange {
    #[tdf(tag = "ALST")]
    pub player_id: PlayerID,
    #[tdf(tag = "GID")]
    pub game_id: GameID,
    #[tdf(tag = "OPER")]
    pub operation: AdminListOperation,
    #[tdf(tag = "UID")]
    pub host_id: PlayerID,
}

/// Different operations that can be performed on
/// the admin list
#[derive(Debug, Clone, Copy, TdfSerialize, TdfTyped)]
#[repr(u8)]
pub enum AdminListOperation {
    Add = 0,
    Remove = 1,
}

#[derive(TdfSerialize)]
pub struct PlayerStateChange {
    #[tdf(tag = "GID")]
    pub gid: GameID,
    #[tdf(tag = "PID")]
    pub pid: PlayerID,
    #[tdf(tag = "STAT")]
    pub state: PlayerState,
}

#[derive(TdfSerialize)]
pub struct JoinComplete {
    #[tdf(tag = "GID")]
    pub game_id: GameID,
    #[tdf(tag = "PID")]
    pub player_id: PlayerID,
}

#[derive(TdfSerialize)]
pub struct HostMigrateStart {
    #[tdf(tag = "GID")]
    pub game_id: GameID,
    #[tdf(tag = "HOST")]
    pub host_id: PlayerID,
    #[tdf(tag = "PMIG")]
    pub pmig: u32,
    #[tdf(tag = "SLOT")]
    pub slot: u8,
}

#[derive(TdfSerialize)]
pub struct HostMigrateFinished {
    #[tdf(tag = "GID")]
    pub game_id: GameID,
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

impl TdfSerialize for AsyncMatchmakingStatus {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.tag_list_start(b"ASIL", TdfType::Group, 1);
        w.group_body(|w| {
            // Create game status
            w.group(b"CGS", |w| {
                // Evaluate status
                // PlayerCountSufficient = 1,
                // AcceptableHostFound = 2,
                // TeamSizesSufficient = 4
                w.tag_u8(b"EVST", 2 | 4);
                // Number of matchmaking sessions
                w.tag_u8(b"MMSN", 1);
                // Number of matched players
                w.tag_u8(b"NOMP", 0);
            });

            // Custom async status
            w.tag_group_empty(b"CUST");

            // DNF rule status
            w.group(b"DNFS", |w| {
                // My DNF value
                w.tag_zero(b"MDNF");
                // Max DNF value
                w.tag_zero(b"XDNF");
            });

            // Find game status
            w.group(b"FGS", |w| {
                // Number of games
                w.tag_zero(b"GNUM");
            });

            // Geo location rule status
            w.group(b"GEOS", |w| {
                // Max distance
                w.tag_zero(b"DIST");
            });

            // Generic rule status dictionary (TODO: RULES HERE)
            w.tag_map_start(b"GRDA", TdfType::String, TdfType::Group, 0);

            // Game size rule status
            w.group(b"GSRD", |w| {
                // Max player count accepted
                w.tag_u8(b"PMAX", 4);
                // Min player count accepted
                w.tag_u8(b"PMIN", 2);
            });

            // Host balance rule status
            w.group(b"HBRD", |w| {
                // Host balance values
                // HOSTS_STRICTLY_BALANCED = 0,
                // HOSTS_BALANCED = 1,
                // HOSTS_UNBALANCED = 2,

                w.tag_u8(b"BVAL", 1);
            });

            // Host viability rule status
            w.group(b"HVRD", |w| {
                // Host viability values
                // CONNECTION_ASSURED = 0,
                // CONNECTION_LIKELY = 1,
                // CONNECTION_FEASIBLE = 2,
                // CONNECTION_UNLIKELY = 3,

                w.tag_zero(b"VVAL");
            });

            // Ping site rule status
            w.group(b"PSRS", |_| {});

            // Rank rule status
            w.group(b"RRDA", |w| {
                // Matched rank flags
                w.tag_zero(b"RVAL");
            });

            // Team size rule status
            w.group(b"TSRS", |w| {
                // Max team size accepted
                w.tag_zero(b"TMAX");
                // Min team size accepted
                w.tag_zero(b"TMIN");
            });

            // UED rule status
            w.tag_map_empty(b"GRDA", TdfType::String, TdfType::Group);
            // Virtual game rule status
            w.group(b"VGRS", |w| w.tag_zero(b"VVAL"));
        });

        w.tag_owned(b"MSID", self.player_id);
        w.tag_owned(b"USID", self.player_id);
    }
}

#[derive(
    Default, Debug, Serialize, Clone, Copy, PartialEq, Eq, TdfDeserialize, TdfSerialize, TdfTyped,
)]
#[repr(u8)]
pub enum PlayerState {
    /// Link between the mesh points is not connected
    #[default]
    #[tdf(default)]
    Reserved = 0x0,
    Queued = 0x1,
    /// Link is being formed between two mesh points
    ActiveConnecting = 0x2,
    ActiveMigrating = 0x3,
    /// Link is connected between two mesh points
    ActiveConnected = 0x4,
    ActiveKickPending = 0x5,
}

/// Message for a game state changing
#[derive(TdfSerialize)]
pub struct StateChange {
    /// The ID of the game
    #[tdf(tag = "GID")]
    pub id: GameID,
    /// The game state
    #[tdf(tag = "GSTA")]
    pub state: GameState,
}

/// Message for a game setting changing
#[derive(TdfSerialize)]
pub struct SettingChange {
    /// The game setting
    #[tdf(tag = "ATTR", into = u16)]
    pub settings: GameSettings,
    /// The ID of the game
    #[tdf(tag = "GID")]
    pub id: GameID,
}

/// Packet for game attribute changes
pub struct AttributesChange<'a> {
    /// Borrowed game attributes map
    pub attributes: &'a AttrMap,
    /// The id of the game the attributes have changed for
    pub id: GameID,
}

impl TdfSerialize for AttributesChange<'_> {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.tag_ref(b"ATTR", self.attributes);
        w.tag_owned(b"GID", self.id);
    }
}

/// Message for a player joining notification
pub struct PlayerJoining<'a> {
    /// The ID of the game
    pub game_id: GameID,
    /// The slot the player is joining into
    pub slot: usize,
    /// The player that is joining
    pub player: &'a GamePlayer,
}

impl TdfSerialize for PlayerJoining<'_> {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.tag_u32(b"GID", self.game_id);

        w.tag_group(b"PDAT");
        self.player.encode(self.game_id, self.slot, w);
    }
}

/// Different states the game can be in
#[derive(
    Default, Debug, Serialize, Clone, Copy, PartialEq, Eq, TdfSerialize, TdfDeserialize, TdfTyped,
)]
#[repr(u8)]
pub enum GameState {
    NewState = 0x0,
    #[tdf(default)]
    #[default]
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

impl From<GameSettings> for u16 {
    fn from(value: GameSettings) -> Self {
        value.bits()
    }
}

impl From<u16> for GameSettings {
    fn from(value: u16) -> Self {
        GameSettings::from_bits_retain(value)
    }
}

const VSTR: &str = "ME3-295976325-179181965240128";

#[derive(TdfSerialize, TdfTyped)]
pub enum GameSetupContext {
    /// Context without additional data
    #[tdf(key = 0x0, tag = "VALU")]
    Dataless {
        #[tdf(tag = "DCTX")]
        context: DatalessContext,
    },
    /// Context added from matchmaking
    #[tdf(key = 0x3, tag = "VALU")]
    Matchmaking {
        #[tdf(tag = "FIT")]
        fit_score: u16,
        #[tdf(tag = "MAXF")]
        max_fit_score: u16,
        #[tdf(tag = "MSID")]
        session_id: PlayerID,
        #[tdf(tag = "RSLT")]
        result: MatchmakingResult,
        #[tdf(tag = "USID")]
        player_id: PlayerID,
    },
}

#[derive(Debug, Copy, Clone, TdfSerialize, TdfTyped)]
#[repr(u8)]
pub enum MatchmakingResult {
    // CreatedGame = 0x0,
    // JoinedNewGame = 0x1,
    JoinedExistingGame = 0x2,
    // TimedOut = 0x3,
    // Canceled = 0x4,
    // Terminated = 0x5,
    // GameSetupFailed = 0x6,
}

#[derive(Debug, Copy, Clone, TdfSerialize, TdfTyped)]
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

pub struct GameSetupResponse<'a> {
    pub game: &'a Game,
    pub context: GameSetupContext,
}

impl TdfSerialize for GameSetupResponse<'_> {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        let game = self.game;
        let host = game.players.first().expect("Missing game host for setup");

        w.group(b"GAME", |w| {
            w.tag_list_iter_owned(b"ADMN", game.players.iter().map(|player| player.player.id));
            w.tag_ref(b"ATTR", &game.attributes);
            w.tag_list_slice::<u8>(b"CAP", &[4, 0]);
            w.tag_u32(b"GID", game.id);
            w.tag_str(b"GNAM", &host.player.display_name);
            w.tag_u64(b"GPVH", 0x5a4f2b378b715c6);
            w.tag_owned(b"GSET", game.settings.bits());
            w.tag_u64(b"GSID", 0x4000000a76b645);
            w.tag_ref(b"GSTA", &game.state);

            w.tag_str_empty(b"GTYP");
            {
                w.tag_list_start(b"HNET", TdfType::Group, 1);
                w.write_byte(2);
                if let NetworkAddress::AddressPair(pair) = &host.net.addr {
                    TdfSerialize::serialize(pair, w)
                }
            }

            w.tag_u32(b"HSES", host.player.id);
            w.tag_zero(b"IGNO");
            w.tag_u8(b"MCAP", 4);
            w.tag_ref(b"NQOS", &host.net.qos);
            w.tag_zero(b"NRES");
            w.tag_zero(b"NTOP");
            w.tag_str_empty(b"PGID");
            w.tag_blob_empty(b"PGSR");

            // Platform host info
            w.group(b"PHST", |w| {
                w.tag_u32(b"HPID", host.player.id);
                w.tag_zero(b"HSLT");
            });

            w.tag_u8(b"PRES", 0x1);
            w.tag_str_empty(b"PSAS");
            // Queue capacity
            w.tag_zero(b"QCAP");
            // Shared game randomness seed?
            w.tag_u32(b"SEED", 0x4cbc8585);
            // tEAM capacity
            w.tag_zero(b"TCAP");

            // Topology host info
            w.group(b"THST", |w| {
                w.tag_u32(b"HPID", host.player.id);
                w.tag_zero(b"HSLT");
            });

            w.tag_str(b"UUID", "286a2373-3e6e-46b9-8294-3ef05e479503");
            w.tag_u8(b"VOIP", 0x2);
            w.tag_str(b"VSTR", VSTR);
            w.tag_blob_empty(b"XNNC");
            w.tag_blob_empty(b"XSES");
        });

        // Player list
        w.tag_list_start(b"PROS", TdfType::Group, game.players.len());
        for (slot, player) in game.players.iter().enumerate() {
            player.encode(game.id, slot, w);
        }

        w.tag_ref(b"REAS", &self.context);
    }
}

pub struct GetGameDetails<'a> {
    pub game: &'a Game,
}

impl TdfSerialize for GetGameDetails<'_> {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        let game = self.game;
        let host = game.players.first().expect("Missing game host for details");

        w.tag_list_start(b"GDAT", TdfType::Group, 1);
        w.group_body(|w| {
            w.tag_list_iter_owned(b"ADMN", game.players.iter().map(|player| player.player.id));
            w.tag_ref(b"ATTR", &game.attributes);
            w.tag_list_slice(b"CAP", &[4u8, 0u8]);

            w.tag_u32(b"GID", game.id);
            w.tag_str(b"GNAM", &host.player.display_name);
            w.tag_u16(b"GSET", game.settings.bits());
            w.tag_ref(b"GSTA", &game.state);
            {
                w.tag_list_start(b"HNET", TdfType::Group, 1);
                w.write_byte(2);
                if let NetworkAddress::AddressPair(pair) = &host.net.addr {
                    TdfSerialize::serialize(pair, w)
                }
            }
            w.tag_u32(b"HOST", host.player.id);
            w.tag_zero(b"NTOP");

            w.tag_list_slice(b"PCNT", &[1u8, 0u8]);

            w.tag_u8(b"PRES", 0x2);
            w.tag_str(b"PSAS", "ea-sjc");
            w.tag_str_empty(b"PSID");
            w.tag_zero(b"QCAP");
            w.tag_zero(b"QCNT");
            w.tag_zero(b"SID");
            w.tag_zero(b"TCAP");
            w.tag_u8(b"VOIP", 0x2);
            w.tag_str(b"VSTR", VSTR);
        });
    }
}
