use std::collections::VecDeque;

use log::debug;
use tokio::sync::RwLock;

use crate::blaze::{Session, SessionArc};

use super::{
    enums::{Difficulty, EnemyType, GameMap, MatchRule},
    Game,
};
use super::{GameArc, Games};

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
    pub async fn matches(&self, game: &Game) -> bool {
        let game_data = game.data.read().await;
        let attributes = &game_data.attributes;

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

/// Structure for storing the active matchmaking queue
/// and keeping it updated.
pub struct Matchmaking {
    queue: RwLock<VecDeque<(SessionArc, RuleSet)>>,
}

impl Matchmaking {
    pub fn new() -> Self {
        Self {
            queue: RwLock::new(VecDeque::new()),
        }
    }

    /// Async handler for when a new game is created in order to update
    /// the queue checking if any of the other players rule sets match the
    /// details of the game
    pub async fn on_game_created(&self, game: &GameArc) {
        debug!("Matchmaking game created. Checking queue for players...");
        let mut removed_ids = Vec::new();
        {
            let queue = self.queue.read().await;
            for (session, rules) in queue.iter() {
                if rules.matches(game).await && game.is_joinable().await {
                    debug!("Found player from queue. Adding them to the game.");
                    if let Ok(_) = Game::add_player(game, session).await {
                        removed_ids.push(session.id);
                    } else {
                        break;
                    }
                }
            }
        }

        if removed_ids.len() > 0 {
            let queue = &mut *self.queue.write().await;
            queue.retain(|value| !removed_ids.contains(&value.0.id))
        }
    }

    /// Attempts to find a game that matches the players provided rule set
    /// or adds them to the matchmaking queue if one could not be found.
    pub async fn get_or_queue(
        &self,
        session: &SessionArc,
        rules: RuleSet,
        games: &Games,
    ) -> Option<GameArc> {
        let games = games.games.read().await;
        for game in games.values() {
            if rules.matches(game).await {
                println!("Found matching game {}", &game.name);
                return Some(game.clone());
            }
        }

        // Update the player matchmaking data.
        {
            let session_data = &mut *session.data.write().await;
            session_data.matchmaking = true;
        }

        debug!("Updated player matchmaking data");

        // Push the player to the end of the queue
        let queue = &mut *self.queue.write().await;
        queue.push_back((session.clone(), rules));
        debug!("Added player to back of queue");

        None
    }

    /// Removes a player from the queue if it exists
    pub async fn remove(&self, session: &Session) {
        let queue = &mut *self.queue.write().await;
        queue.retain(|value| value.0.id != session.id);
    }

    pub async fn update(&self) {
        // TODO: Update matchmaking queue with async notifis
    }
}
