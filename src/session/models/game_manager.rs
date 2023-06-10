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
use blaze_pk::{
    codec::{Decodable, Encodable},
    error::DecodeResult,
    reader::TdfReader,
    tag::TdfType,
    writer::TdfWriter,
};

/// Structure of the request for creating new games contains the
/// initial game attributes and game setting
pub struct CreateGameRequest {
    /// The games initial attributes
    pub attributes: AttrMap,
    /// The games initial setting
    pub setting: GameSettings,
}

impl Decodable for CreateGameRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let attributes: AttrMap = reader.tag(b"ATTR")?;
        let setting: GameSettings = reader.tag(b"GSET")?;
        Ok(Self {
            attributes,
            setting,
        })
    }
}

/// Structure for the response to game creation which contains
/// the ID of the created game
pub struct CreateGameResponse {
    /// The game ID
    pub game_id: GameID,
}

impl Encodable for CreateGameResponse {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u32(b"GID", self.game_id);
    }
}

/// Structure of request to remove player from a game
pub struct RemovePlayerRequest {
    /// The ID of the game to remove from
    pub game_id: GameID,
    /// The ID of the player to remove
    pub player_id: PlayerID,
    // The reason the player was removed
    pub reason: RemoveReason,
}

impl Decodable for RemovePlayerRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let game_id: GameID = reader.tag(b"GID")?;
        let player_id: PlayerID = reader.tag(b"PID")?;
        let reason: RemoveReason = reader.tag(b"REAS")?;
        Ok(Self {
            game_id,
            player_id,
            reason,
        })
    }
}

pub struct SetAttributesRequest {
    /// The new game attributes
    pub attributes: AttrMap,
    /// The ID of the game to set the attributes for
    pub game_id: GameID,
}

impl Decodable for SetAttributesRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let attributes = reader.tag(b"ATTR")?;
        let game_id: GameID = reader.tag(b"GID")?;

        Ok(Self {
            attributes,
            game_id,
        })
    }
}

pub struct SetStateRequest {
    pub game_id: GameID,
    pub state: GameState,
}

impl Decodable for SetStateRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let game_id: GameID = reader.tag(b"GID")?;
        let state: GameState = reader.tag(b"GSTA")?;
        Ok(Self { game_id, state })
    }
}
pub struct SetSettingRequest {
    pub game_id: GameID,
    pub setting: GameSettings,
}

impl Decodable for SetSettingRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let game_id: GameID = reader.tag(b"GID")?;
        let setting: GameSettings = reader.tag(b"GSET")?;
        Ok(Self { game_id, setting })
    }
}

/// Request to update the state of a mesh connection between
/// payers.
pub struct UpdateMeshRequest {
    pub game_id: GameID,
    pub target: Option<MeshTarget>,
}

pub struct MeshTarget {
    pub player_id: PlayerID,
    pub state: PlayerState,
}

impl Decodable for UpdateMeshRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let game_id: GameID = reader.tag(b"GID")?;
        let count: usize = reader.until_list(b"TARG", TdfType::Group)?;

        let target = if count > 0 {
            let player_id: PlayerID = reader.tag(b"PID")?;
            let state: PlayerState = reader.tag(b"STAT")?;
            let target = MeshTarget { player_id, state };
            Some(target)
        } else {
            None
        };

        Ok(Self { game_id, target })
    }
}

/// Structure of the request for starting matchmaking. Contains
/// the rule set that games must match in order to join
pub struct MatchmakingRequest {
    /// The matchmaking rule set
    pub rules: RuleSet,
}

impl Decodable for MatchmakingRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        reader.until_tag(b"CRIT", TdfType::Group)?;
        let rule_count: usize = reader.until_list(b"RLST", TdfType::Group)?;

        let mut rules: Vec<(String, String)> = Vec::with_capacity(rule_count);
        for _ in 0..rule_count {
            let name: String = reader.tag(b"NAME")?;
            let values_count: usize = reader.until_list(b"VALU", TdfType::String)?;
            if values_count < 1 {
                continue;
            }
            let value: String = reader.read_string()?;
            if values_count > 1 {
                for _ in 1..rule_count {
                    reader.skip_blob()?;
                }
            }
            reader.skip_group()?;
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
pub struct MatchmakingResponse {
    /// The current session ID
    pub id: SessionID,
}

impl Encodable for MatchmakingResponse {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u32(b"MSID", self.id);
    }
}

pub struct GetGameDataRequest {
    pub game_list: Vec<GameID>,
}

impl Decodable for GetGameDataRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let game_list: Vec<GameID> = reader.tag(b"GLST")?;
        Ok(Self { game_list })
    }
}

pub struct JoinGameRequest {
    /// The join target
    pub target_id: PlayerID,
}

impl Decodable for JoinGameRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        reader.until_tag(b"USER", TdfType::Group)?;
        let target_id: PlayerID = reader.tag(b"ID")?;
        Ok(Self { target_id })
    }
}

pub struct JoinGameResponse {
    pub game_id: GameID,
}

impl Encodable for JoinGameResponse {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u32(b"GID", self.game_id);

        // TODO: Join states: JOINED_GAME = 0, IN_QUEUE = 1, GROUP_PARTIALLY_JOINED = 2
        writer.tag_zero(b"JGS");
    }
}
