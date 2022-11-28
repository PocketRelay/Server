use core::game::{
    codec::{GameState, RemoveReason},
    game::AttrMap,
    rules::{MatchRules, RuleSet},
};

use blaze_pk::{
    codec::{Codec, CodecError, CodecResult, Reader},
    tag::{Tag, ValueType},
    tagging::{expect_list, expect_tag, tag_u32},
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

impl Codec for CreateGameRequest {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let attributes = Tag::expect(reader, "ATTR")?;
        let setting = Tag::expect(reader, "GSET")?;
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

impl Codec for CreateGameResponse {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_u32(output, "GID", self.game_id);
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

impl Codec for RemovePlayerRequest {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let game_id = expect_tag(reader, "GID")?;
        let player_id = expect_tag(reader, "PID")?;
        let reason = expect_tag(reader, "REAS")?;
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

impl Codec for GameModifyRequest {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let first = Tag::decode(reader)?;
        let first_name: &str = &first.0;
        let game_id = match first_name {
            "ATTR" => {
                let attributes = AttrMap::decode(reader)?;
                let game_id = Tag::expect(reader, "GID")?;
                return Ok(Self::Attributes(game_id, attributes));
            }
            "GID" => GameID::decode(reader)?,
            _ => return Err(CodecError::Other("Unknown game modify attribute")),
        };
        let value_tag = Tag::decode(reader)?;
        let tag: &str = &value_tag.0;
        Ok(match tag {
            "GSTA" => {
                let state = GameState::decode(reader)?;
                Self::State(game_id, state)
            }
            "GSET" => {
                let setting = u16::decode(reader)?;
                Self::Setting(game_id, setting)
            }
            _ => return Err(CodecError::Other("Missing modify contents")),
        })
    }
}

/// Request to update the state of a mesh connection between
/// payers.
pub struct UpdateMeshRequest {
    pub game_id: GameID,
    pub targets: Vec<PlayerID>,
}

impl Codec for UpdateMeshRequest {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let game_id = Tag::expect(reader, "GID")?;
        let count = expect_list(reader, "TARG", ValueType::Group)?;
        let mut targets = Vec::with_capacity(count);
        for _ in 0..count {
            let player_id = Tag::expect(reader, "PID")?;
            targets.push(player_id);
            Tag::discard_group(reader)?;
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

impl Codec for MatchmakingRequest {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        Tag::decode_until(reader, "CRIT", ValueType::Group)?;
        let rule_count = expect_list(reader, "RLST", ValueType::Group)?;
        let mut rules = Vec::new();
        for _ in 0..rule_count {
            let name: String = expect_tag(reader, "NAME")?;
            let values_count = expect_list(reader, "VALU", ValueType::String)?;
            if values_count < 1 {
                continue;
            }
            let value: String = String::decode(reader)?;
            if values_count > 1 {
                for _ in 1..rule_count {
                    String::skip(reader)?;
                }
            }

            Tag::discard_group(reader)?;

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

impl Codec for MatchmakingResponse {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_u32(output, "MSID", self.id);
    }
}
