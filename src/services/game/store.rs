use super::{rules::RuleSet, Game, GameJoinableState, GameRef, GameSnapshot};
use crate::utils::{hashing::IntHashMap, types::GameID};
use parking_lot::RwLock;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

pub struct Games {
    /// Stored value for the ID to give the next game
    next_id: AtomicU32,
    /// The map of games to the actual game address
    games: RwLock<IntHashMap<GameID, GameRef>>,
}

impl Default for Games {
    fn default() -> Self {
        Self {
            next_id: AtomicU32::new(1),
            games: Default::default(),
        }
    }
}

impl Games {
    /// Obtains the total count of games in the list
    pub fn total(&self) -> usize {
        self.games.read().len()
    }

    pub fn remove_by_id(&self, game_id: GameID) {
        _ = self.games.write().remove(&game_id);
    }

    pub fn get_by_id(&self, game_id: GameID) -> Option<GameRef> {
        self.games.read().get(&game_id).cloned()
    }

    /// Find a game that matches the provided rule set
    pub fn get_by_rule_set(&self, rule_set: &RuleSet) -> Option<(GameID, GameRef)> {
        self.games
            .read()
            .iter()
            .find(|(_game_id, game_ref)| {
                let join_state = game_ref.read().joinable_state(Some(rule_set));
                matches!(join_state, GameJoinableState::Joinable)
            })
            .map(|(game_id, game_ref)| (*game_id, game_ref.clone()))
    }

    // Get the next available game ID
    pub fn next_id(&self) -> GameID {
        self.next_id.fetch_add(1, Ordering::AcqRel)
    }

    pub fn insert(&self, game: Game) -> GameRef {
        let game_id = game.id;
        let link = Arc::new(RwLock::new(game));
        self.games.write().insert(game_id, link.clone());
        link
    }

    /// Creates a snapshot of the state of the current games
    pub fn create_snapshot(
        &self,
        offset: usize,
        count: usize,
        include_net: bool,
        include_players: bool,
    ) -> (Vec<GameSnapshot>, bool) {
        let games = &*self.games.read();

        // Create an ordered set
        let mut items: Vec<(&GameID, &GameRef)> = games.iter().collect();
        items.sort_by_key(|(key, _)| *key);

        // Whether there is more keys that what was requested
        let more = items.len() > offset + count;

        // Take snapshot of each game state
        let snapshots: Vec<GameSnapshot> = items
            .into_iter()
            // Skip to the desired offset
            .skip(offset)
            // Take the desired number of keys
            .take(count)
            // Iterate over the game links
            .map(|(_, value)| value.clone())
            // Spawn the snapshot tasks
            .map(|game| game.read().snapshot(include_net, include_players))
            .collect();

        (snapshots, more)
    }
}
