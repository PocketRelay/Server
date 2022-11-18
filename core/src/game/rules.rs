use log::debug;

use super::game::AttrMap;

use super::enums::{Difficulty, EnemyType, GameMap, MatchRule};

// DLC Requirement attributes
// ME3_dlc2300 = required
// ME3_dlc2500
// ME3_dlc2700
// ME3_dlc3050
// ME3_dlc3225

/// Structure for known rule types that can be compared
/// correctly. Unknown rules are simply ignored.
#[derive(Debug, PartialEq, Eq)]
pub enum MatchRules {
    Map(GameMap),
    Enemy(EnemyType),
    Difficulty(Difficulty),
}

impl MatchRules {
    /// Parses a match rule from the provided key and value pair
    /// the key being the rule key present in the matchmaking query.
    pub fn parse(key: &str, value: &str) -> Option<Self> {
        Some(match key {
            GameMap::RULE => Self::Map(GameMap::from_value(value)),
            EnemyType::RULE => Self::Enemy(EnemyType::from_value(value)),
            Difficulty::RULE => Self::Difficulty(Difficulty::from_value(value)),
            _ => return None,
        })
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
    /// Creates a new rule set from the provided vec of match rules.
    pub fn new(values: Vec<MatchRules>) -> Self {
        Self { values }
    }

    /// Attempts to see if the provided game matches the rules
    /// in this rule set. Its okay for the values of rules to be
    /// missing and rules with unknown values are treated as a
    /// failure.
    pub fn matches(&self, attributes: &AttrMap) -> bool {
        for rule in &self.values {
            let attr = rule.attr();
            if let Some(value) = attributes.get(attr) {
                debug!("Comparing {rule:?} {attr} {value}");
                if !rule.try_compare(value) {
                    debug!("Doesn't Match");
                    return false;
                } else {
                    debug!("Matches")
                }
            } else {
                debug!("Game didn't have attr {rule:?} {attr}");
            }
        }

        true
    }
}
