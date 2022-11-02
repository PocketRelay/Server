use std::collections::HashMap;
use std::sync::Arc;

use super::{
    enums::{Difficulty, EnemyType, GameMap, MatchRule},
    Game,
};

/// Structure for known rule types that can be compared
/// correctly. Unknown rules are simply ignored.
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum MatchRules {
    Map(GameMap),
    Enemy(EnemyType),
    Difficulty(Difficulty),
}

impl MatchRules {
    /// Parses a match rule from the provided key and value pair
    /// the key being the rule key present in the matchmaking query.
    pub fn parse(key: &str, value: &str) -> Self {
        match key {
            GameMap::RULE => Self::Map(GameMap::from_value(value)),
            EnemyType::RULE => Self::Enemy(EnemyType::from_value(value)),
            Difficulty::RULE => Self::Enemy(Difficulty::from_value(value)),
        }
    }

    /// Function for finding the attribute key for the
    /// provided rule value in a game attributes map.
    pub fn attr(&self) -> &'static str {
        match self {
            Self::Map(_) => GameMap::ATTR,
            Self::Enemy(_) => EnemyType::ATTR,
            Self::Difficulty(_) => Difficulty::ATTR,
        }
    }

    /// Attempts to compare the provided value from the
    /// game attributes map with the value stored in the
    /// underlying enum.
    pub fn try_compare(&self, value: &str) -> bool {
        match self {
            Self::Map(a) => a.try_compare(value),
            Self::Enemy(a) => a.try_compare(value),
            Self::Difficulty(a) => a.try_compare(value),
        }
    }
}

/// Structure for a set of rules used to determine
/// whether a matchmaking query matches a game.
pub struct RuleSet {
    /// The list of match rules.
    values: Vec<MatchRules>,
}

impl RuleSet {
    /// Attempts to see if the provided game matches the rules
    /// in this rule set. Its okay for the values of rules to be
    /// missing and rules with unknown values are treated as a
    /// failure.
    pub async fn matches(&self, game: &Game) -> bool {
        let game_data = game.data.read().await;
        let attributes = &game_data.attributes;

        for rule in self.values {
            let attr = rule.attr();
            if let Some(value) = attributes.get(attr) {
                if !rule.try_compare(value) {
                    return false;
                }
            }
        }

        true
    }
}
