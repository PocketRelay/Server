use tdf::{TdfDeserialize, TdfDeserializeOwned, TdfSerialize, TdfType};

use crate::{
    services::{
        game::{
            models::{GameSettings, GameState, PlayerState, RemoveReason},
            AttrMap,
        },
        matchmaking::rules::RuleSet,
    },
    utils::types::{GameID, PlayerID, SessionID},
};

/// Structure of the request for creating new games contains the
/// initial game attributes and game setting
#[derive(TdfDeserialize)]
pub struct CreateGameRequest {
    /// The games initial attributes
    #[tdf(tag = "ATTR")]
    pub attributes: AttrMap,
    /// The games initial setting
    #[tdf(tag = "GSET")]
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
    #[tdf(tag = "GSET")]
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

#[derive(TdfDeserialize)]
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
            let value: String = r.read_string()?;
            if values_count > 1 {
                for _ in 1..rule_count {
                    r.skip_blob()?;
                }
            }
            r.skip_group()?;
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

#[derive(TdfDeserialize)]
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

#[derive(TdfSerialize)]
#[repr(u8)]
pub enum JoinGameState {
    JoinedGame = 0,
    InQueue = 1,
    GroupPartiallyJoined = 2,
}
