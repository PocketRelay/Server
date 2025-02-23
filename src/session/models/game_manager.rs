use super::{util::PING_SITE_ALIAS, NatType, NetworkAddress};
use crate::{
    config::{Config, TunnelConfig},
    services::{
        game::{rules::RuleSet, AttrMap, Game, GamePlayer},
        tunnel::http_tunnel::TUNNEL_HOST_LOCAL_PORT,
    },
    utils::types::{GameID, PlayerID},
};
use bitflags::bitflags;
use serde::Serialize;
use std::net::Ipv4Addr;
use tdf::{
    types::tagged_union::TAGGED_UNSET_KEY, Blob, GroupSlice, TdfDeserialize, TdfDeserializeOwned,
    TdfSerialize, TdfType, TdfTyped,
};

#[derive(Debug, Clone)]
#[repr(u16)]
#[allow(unused)]
pub enum GameManagerError {
    InvalidGameId = 0x2,
    GameFull = 0x4,
    PermissionDenied = 0x1e,
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
    pub targets: Vec<PlayerConnectionStatus>,
}

#[derive(TdfDeserialize, TdfTyped)]
#[tdf(group)]
pub struct PlayerConnectionStatus {
    #[tdf(tag = "PID")]
    pub player_id: PlayerID,
    #[tdf(tag = "STAT")]
    pub status: PlayerNetConnectionStatus,
}

#[derive(TdfDeserialize, TdfTyped)]
#[repr(u8)]
pub enum PlayerNetConnectionStatus {
    Disconnected = 0x0,
    EstablishingConnection = 0x1,
    Connected = 0x2,
}

#[derive(TdfDeserialize)]
pub struct AddAdminPlayerRequest {
    #[tdf(tag = "GID")]
    pub game_id: GameID,
    #[tdf(tag = "PID")]
    pub player_id: PlayerID,
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
    pub id: PlayerID,
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
    /// Player lost PTP connection
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
#[derive(TdfSerialize)]
pub struct AttributesChange<'a> {
    /// Borrowed game attributes map
    #[tdf(tag = "ATTR")]
    pub attributes: &'a AttrMap,
    /// The id of the game the attributes have changed for
    #[tdf(tag = "GID")]
    pub id: GameID,
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
    /// Data structure just created
    NewState = 0x0,
    /// Closed to joins/matchmaking
    #[tdf(default)]
    #[default]
    Initializing = 0x1,
    /// Game will need topology host assigned when player joins.
    InactiveVirtual = 0x2,
    /// Game created via matchmaking is waiting for connections to be established and validated.
    ConnectionVerification = 0x3,
    /// Pre game state, obey joinMode flags
    PreGame = 0x82,
    /// Game available, obey joinMode flag
    InGame = 0x83,
    /// After game is done,closed to joins/matchmaking
    PostGame = 0x4,
    /// Game migration state, closed to joins/matchmaking
    Migrating = 0x5,
    /// Game destruction state, closed to joins/matchmaking
    Destructing = 0x6,
    /// Game resettable state, closed to joins/matchmaking, but available to be reset
    Resettable = 0x7,
    /// Unresponsive, closed to joins/matchmaking
    Unresponsive = 0x9,
    /// Initialized state, intended for the use of game group
    GameGroupInitialized = 0x10,
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
        const SEND_ORPHANED_GAME_REPORT_EVENT = 0x4000;
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

const GAME_PROTOCOL_VERSION: &str = "ME3-295976325-179181965240128";

/// UNSPECIFIED_TEAM_INDEX will assign the player to whichever team has room.
pub const UNSPECIFIED_TEAM_INDEX: u16 = 0xffff;

/// Game version hashing
///
/// Credits to Aim4kill https://github.com/PocketRelay/Server/issues/59
fn compute_version_hash(version: &str) -> u64 {
    const OFFSET: u64 = 2166136261;
    const PRIME: u64 = 16777619;

    version
        .as_bytes()
        .iter()
        .copied()
        .fold(OFFSET, |hash, byte| {
            (hash.wrapping_mul(PRIME)) ^ (byte as u64)
        })
}

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

#[allow(unused)]
#[derive(Debug, Copy, Clone, TdfSerialize, TdfTyped)]
#[repr(u8)]
pub enum PresenceMode {
    // No presence management. E.g. For games that should never be advertised in shell UX and cannot be used for 1st party invites.
    None = 0x0,
    // Full presence as defined by the platform.
    Standard = 0x1,
    // Private presence as defined by the platform. For private games which are closed to uninvited/outside users.
    Private = 0x2,
}

#[allow(unused)]
#[derive(Debug, Copy, Clone, TdfSerialize, TdfTyped)]
#[repr(u8)]
pub enum VoipTopology {
    /// VOIP is disabled (for a game)
    Disabled = 0x0,
    // /// VOIP uses a star topology; typically some form of 3rd party server dedicated to mixing/broadcasting voip streams.
    // DedicatedServer = 0x1
    /// VOIP uses a full mesh topology; each player makes peer connections to the other players/members for voip traffic.
    PeerToPeer = 0x2,
}

#[allow(unused)]
#[derive(Debug, Copy, Clone, TdfSerialize, TdfTyped)]
#[repr(u8)]
pub enum GameNetworkTopology {
    /// client server peer hosted network topology
    PeerHosted = 0x0,
    /// client server dedicated server topology
    Dedicated = 0x1,
    /// Peer to peer full mesh network topology
    FullMesh = 0x82,
    /// Networking is disabled??
    Disabled = 0xFF,
}

#[allow(unused)]
#[derive(Debug, Copy, Clone, TdfSerialize, TdfTyped)]
#[repr(u8)]
pub enum SlotType {
    // Public participant slot, usable by any participant
    PublicParticipant = 0x0,
    // Private participant slot, reserved for invited participant
    PrivateParticipant = 0x1,
}

pub struct GameSetupResponse<'a> {
    pub game: &'a Game,
    pub context: GameSetupContext,
    pub config: &'a Config,
}

impl TdfSerialize for GameSetupResponse<'_> {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        let game = self.game;
        let host = game.players.first().expect("Missing game host for setup");

        w.group(b"GAME", |w| {
            // Admin player list
            w.tag_list_iter_owned(b"ADMN", game.players.iter().map(|player| player.player.id));
            // Game attributes
            w.tag_ref(b"ATTR", &game.attributes);
            // Slot Capacities
            w.tag_list_slice::<usize>(
                b"CAP",
                &[
                    Game::MAX_PLAYERS, /* Public slots */
                    0,                 /* Private slots */
                ],
            );
            // Game ID
            w.tag_u32(b"GID", game.id);
            // Game Name
            w.tag_str(b"GNAM", &host.player.display_name);
            // Game Protocol Version Hash
            w.tag_u64(b"GPVH", compute_version_hash(GAME_PROTOCOL_VERSION));
            // Game settings
            w.tag_owned(b"GSET", game.settings.bits());
            // Game Reporting ID
            w.tag_u64(b"GSID", 0x4000000a76b645);
            // Game state
            w.tag_ref(b"GSTA", &game.state);
            // Game Type used for game reporting as passed up in the request.
            w.tag_str_empty(b"GTYP");

            // Whether to tunnel the connection
            let tunnel = match &self.config.tunnel {
                TunnelConfig::Stricter => !matches!(host.net.qos.natt, NatType::Open),
                TunnelConfig::Always => true,
                TunnelConfig::Disabled => false,
            };

            {
                // Topology host network list (The heat bug is present so this encoded as a group even though its a union)
                w.tag_list_start(b"HNET", TdfType::Group, 1);

                // Override for tunneling
                if tunnel {
                    // Forced local host for test dedicated server
                    w.write_byte(3);
                    TdfSerialize::serialize(
                        &super::PairAddress {
                            addr: Ipv4Addr::LOCALHOST,
                            port: TUNNEL_HOST_LOCAL_PORT,
                        },
                        w,
                    );
                } else {
                    // Open NATs can directly have players connect normally
                    if let NetworkAddress::AddressPair(pair) = &host.net.addr {
                        w.write_byte(2 /* Address pair type */);
                        TdfSerialize::serialize(pair, w)
                    } else {
                        // Uh oh.. host networking is missing...?
                        w.write_byte(TAGGED_UNSET_KEY);
                        w.write_byte(0);
                    }
                }
            }

            // Host session ID
            w.tag_u32(b"HSES", host.player.id);
            w.tag_zero(b"IGNO");

            // Max player capacity
            w.tag_usize(b"MCAP", Game::MAX_PLAYERS);

            // Host network qos data
            w.tag_ref(b"NQOS", &host.net.qos);

            // Flag to indicate that this game is not resettable. This applies only to the CLIENT_SERVER_DEDICATED topology.
            // The game will be prevented from ever going into the RESETTABlE state.
            w.tag_bool(b"NRES", false);

            // Game network topology
            w.tag_alt(
                b"NTOP",
                if tunnel {
                    GameNetworkTopology::Dedicated
                } else {
                    GameNetworkTopology::PeerHosted
                },
            );

            // Persisted Game id for the game, used only when game setting's enablePersistedGameIds is true.
            w.tag_str_empty(b"PGID");
            // Persisted Game id secret for the game, used only when game setting's enablePersistedGameIds is true.
            w.tag_blob_empty(b"PGSR");

            // Platform host info
            w.group(b"PHST", |w| {
                w.tag_u32(b"HPID", host.player.id);
                w.tag_zero(b"HSLT");
            });

            // Presence mode
            w.tag_alt(b"PRES", PresenceMode::Standard);
            // Ping site alias
            w.tag_str(b"PSAS", PING_SITE_ALIAS);
            // Queue capacity
            w.tag_zero(b"QCAP");
            // Shared game randomness seed? (a 32 bit number shared between clients)
            // TODO: Randomly generate this when creating a game?
            w.tag_u32(b"SEED", 0x4cbc8585);
            // Team capacity
            w.tag_zero(b"TCAP");

            // The topology host for the game (everyone connects to this person).
            w.group(b"THST", |w| {
                // Player ID
                w.tag_u32(b"HPID", host.player.id);
                // Slot ID
                w.tag_zero(b"HSLT");
            });

            w.tag_str(b"UUID", "286a2373-3e6e-46b9-8294-3ef05e479503");
            // VOIP type
            w.tag_alt(b"VOIP", VoipTopology::PeerToPeer);

            // Game Protocol Version
            w.tag_str(b"VSTR", GAME_PROTOCOL_VERSION);

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
            // Admin player list
            w.tag_list_iter_owned(b"ADMN", game.players.iter().map(|player| player.player.id));
            // Game attributes
            w.tag_ref(b"ATTR", &game.attributes);

            // Slot Capacities
            w.tag_list_slice::<usize>(
                b"CAP",
                &[
                    Game::MAX_PLAYERS, /* Public slots */
                    0,                 /* Private slots */
                ],
            );

            // Game ID
            w.tag_u32(b"GID", game.id);
            // Game name
            w.tag_str(b"GNAM", &host.player.display_name);
            // Game setting
            w.tag_u16(b"GSET", game.settings.bits());
            // Game state
            w.tag_ref(b"GSTA", &game.state);
            {
                // Topology host network list (The heat bug is present so this encoded as a group even though its a union)
                w.tag_list_start(b"HNET", TdfType::Group, 1);

                if let NetworkAddress::AddressPair(pair) = &host.net.addr {
                    w.write_byte(2 /* Address pair type */);
                    TdfSerialize::serialize(pair, w)
                } else {
                    // Uh oh.. host networking is missing...?
                    w.write_byte(TAGGED_UNSET_KEY);
                    w.write_byte(0);
                }
            }
            // Host player ID
            w.tag_u32(b"HOST", host.player.id);

            // Game network topology
            w.tag_alt(b"NTOP", GameNetworkTopology::PeerHosted);

            // Player counts by slot
            w.tag_list_slice::<usize>(
                b"PCNT",
                &[
                    game.players.len(), /* Public count */
                    0,                  /* Private count */
                ],
            );

            // Presence mode
            w.tag_alt(b"PRES", PresenceMode::Standard);

            // Ping site alias
            w.tag_str(b"PSAS", PING_SITE_ALIAS);

            // Persisted Game id for the game, used only when game setting's enablePersistedGameIds is true.
            w.tag_str_empty(b"PGID");

            // Max queue capacity.
            w.tag_zero(b"QCAP");
            // Current number of player in the queue.
            w.tag_zero(b"QCNT");
            // External session ID
            w.tag_zero(b"SID");
            // Team capacity
            w.tag_zero(b"TCAP");
            // VOIP type
            w.tag_alt(b"VOIP", VoipTopology::PeerToPeer);
            // Game Protocol Version
            w.tag_str(b"VSTR", GAME_PROTOCOL_VERSION);
        });
    }
}

#[cfg(test)]
mod test {
    use super::compute_version_hash;

    /// Ensure the version hashing algorithm produces the correct result
    #[test]
    fn test_compute_version_hash() {
        let input: &str = "ME3-295976325-179181965240128";
        let expected: u64 = 0x5a4f2b378b715c6;
        let output: u64 = compute_version_hash(input);
        assert_eq!(output, expected);
    }
}
