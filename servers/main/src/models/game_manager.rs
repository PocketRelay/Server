use blaze_pk::{
    codec::{Decodable, Encodable},
    error::{DecodeError, DecodeResult},
    reader::TdfReader,
    tag::{Tag, TdfType},
    writer::TdfWriter,
};
use core::game::{
    codec::{GameState, RemoveReason},
    game::AttrMap,
    rules::{MatchRules, RuleSet},
};
use utils::types::{GameID, PlayerID, SessionID};

/// Structure of the request for creating new games contains the
/// initial game attributes and game setting
pub struct CreateGameRequest {
    /// The games initial attributes
    pub attributes: AttrMap,
    /// The games initial setting
    pub setting: u16,
}

impl Decodable for CreateGameRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let attributes: AttrMap = reader.tag("ATTR")?;
        let setting: u16 = reader.tag("GSET")?;
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
        let game_id: GameID = reader.tag("GID")?;
        let player_id: PlayerID = reader.tag("PID")?;
        let reason: RemoveReason = reader.tag("REAS")?;
        Ok(Self {
            game_id,
            player_id,
            reason,
        })
    }
}

/// Structure of a request to modify some aspect of a game.
/// This includes the state, setting, and attributes
pub enum GameModifyRequest {
    /// The game state
    State(GameID, GameState),
    /// The game setting
    Setting(GameID, u16),
    /// The game attributes
    Attributes(GameID, AttrMap),
}

impl Decodable for GameModifyRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let first: Tag = reader.read_tag()?;
        let first_name: &str = &first.0;
        let game_id = match first_name {
            "ATTR" => {
                let attributes: AttrMap = AttrMap::decode(reader)?;
                let game_id: GameID = reader.tag("GID")?;
                return Ok(Self::Attributes(game_id, attributes));
            }
            "GID" => GameID::decode(reader)?,
            _ => return Err(DecodeError::Other("Unknown game modify attribute")),
        };
        let value_tag: Tag = reader.read_tag()?;
        let tag: &str = &value_tag.0;
        Ok(match tag {
            "GSTA" => {
                let state: GameState = GameState::decode(reader)?;
                Self::State(game_id, state)
            }
            "GSET" => {
                let setting: u16 = reader.read_u16()?;
                Self::Setting(game_id, setting)
            }
            _ => return Err(DecodeError::Other("Missing modify contents")),
        })
    }
}

/// Request to update the state of a mesh connection between
/// payers.
pub struct UpdateMeshRequest {
    pub game_id: GameID,
    pub targets: Vec<PlayerID>,
}

impl Decodable for UpdateMeshRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let game_id: GameID = reader.tag("GID")?;
        let count: usize = reader.until_list("TARG", TdfType::Group)?;
        let mut targets: Vec<PlayerID> = Vec::with_capacity(count);
        for _ in 0..count {
            let player_id: PlayerID = reader.tag("PID")?;
            targets.push(player_id);
            reader.skip_group()?;
        }
        Ok(Self { game_id, targets })
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
        reader.until_tag("CRIT", TdfType::Group)?;
        let rule_count: usize = reader.until_list("RLST", TdfType::Group)?;
        let mut rules: Vec<MatchRules> = Vec::with_capacity(rule_count);
        for _ in 0..rule_count {
            let name: String = reader.tag("NAME")?;
            let values_count: usize = reader.until_list("VALU", TdfType::String)?;
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
            if let Some(rule) = MatchRules::parse(&name, &value) {
                rules.push(rule);
            }
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
