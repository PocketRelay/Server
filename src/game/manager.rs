use super::{
    player::GamePlayer, rules::RuleSet, Game, GameModifyAction, GameSnapshot, RemovePlayerType,
};
use crate::utils::types::{GameID, SessionID};
use blaze_pk::types::TdfMap;
use log::debug;
use std::{
    collections::{HashMap, VecDeque},
    sync::atomic::{AtomicU32, Ordering},
    time::SystemTime,
};
use tokio::sync::{Mutex, RwLock};

/// Structure for managing games and the matchmaking queue
pub struct Games {
    /// Map of Game IDs to the actual games.
    games: RwLock<HashMap<GameID, Game>>,
    /// Queue of players wanting to join games
    queue: Mutex<VecDeque<QueueEntry>>,
    /// ID for the next game to create
    id: AtomicU32,
}

/// Structure for a entry in the matchmaking queue
struct QueueEntry {
    /// The session that is waiting in the queue
    player: GamePlayer,
    /// The rules that games must meet for this
    /// queue entry to join.
    rules: RuleSet,
    /// The time that the queue entry was created at
    time: SystemTime,
}

impl Default for Games {
    fn default() -> Self {
        Self {
            games: Default::default(),
            queue: Default::default(),
            id: AtomicU32::new(1),
        }
    }
}

impl Games {
    /// Takes a snapshot of all the current games for serialization
    pub async fn snapshot(&self) -> Vec<GameSnapshot> {
        let games = &*self.games.read().await;
        let snapshots = games
            .iter()
            .map(|value| value.1.snapshot())
            .collect::<Vec<_>>();
        futures_util::future::join_all(snapshots).await
    }

    /// Takes a snapshot of the game with the provided game ID
    ///
    /// `game_id` The ID of the game to take the snapshot of
    pub async fn snapshot_id(&self, game_id: GameID) -> Option<GameSnapshot> {
        let games = &*self.games.read().await;
        let game = games.get(&game_id)?;
        Some(game.snapshot().await)
    }

    /// Creates a new game from the initial attributes and
    /// settings provided returning the Game ID of the created
    /// game
    pub async fn create_game(&self, attributes: TdfMap<String, String>, setting: u16) -> u32 {
        let games = &mut *self.games.write().await;
        let id = self.id.fetch_add(1, Ordering::AcqRel);
        let game = Game::new(id, attributes, setting);
        games.insert(id, game);
        id
    }

    /// Adds the host session to the game with the provided game
    /// ID. The game will be compared against any players waiting
    /// in the matchmaking queue.
    ///
    /// `game_id` The ID of the game to add the session to
    /// `session` The session to add as the host
    pub async fn add_host(&self, game_id: GameID, player: GamePlayer) {
        let games = &*self.games.read().await;
        let Some(game) = games.get(&game_id) else { return; };
        game.add_player(player).await;
        self.update_queue(game).await;
    }

    /// Updates the matchmaking queue for the provided game. Will look through
    /// the queue checking if the player rules match the game attributes and if
    /// they do then add them to the game.
    ///
    /// `game` The game to update to queue with
    async fn update_queue(&self, game: &Game) {
        let game_data = game.data.read().await;
        let attributes = &game_data.attributes;

        let queue = &mut *self.queue.lock().await;

        if !queue.is_empty() {
            let mut unmatched = VecDeque::new();
            while let Some(entry) = queue.pop_front() {
                // If the game is not joinable push the entry back to the
                // front of the queue and early return
                if !game.is_joinable().await {
                    queue.push_front(entry);
                    return;
                }

                if entry.rules.matches(attributes) {
                    debug!(
                        "Found player from queue adding them to the game (GID: {})",
                        game.id
                    );
                    let time = SystemTime::now();
                    let elapsed = time.duration_since(entry.time);
                    if let Ok(elapsed) = elapsed {
                        debug!("Matchmaking time elapsed: {}s", elapsed.as_secs())
                    }
                    game.add_player(entry.player).await;
                } else {
                    // TODO: Check started time and timeout
                    // player if they've been waiting too long
                    unmatched.push_back(entry);
                }
            }
            *queue = unmatched;
        }
    }

    /// Attempts to find a game matching the rules provided by the session and
    /// add that player to the game or if there are no matching games to instead
    /// push the player to the matchmaking queue. Will return true if a game was
    /// joined and false if queued.
    ///
    /// `session` The session to get the game for
    /// `rules`   The rules the game must match to be valid
    pub async fn add_or_queue(&self, player: GamePlayer, rules: RuleSet) -> bool {
        let games = &*self.games.read().await;
        for game in games.values() {
            if !game.is_joinable().await {
                continue;
            }
            let game_data = game.data.read().await;
            if rules.matches(&game_data.attributes) {
                debug!("Found matching game (GID: {})", game.id);
                game.add_player(player).await;
                return true;
            }
        }

        let queue = &mut self.queue.lock().await;
        queue.push_back(QueueEntry {
            player,
            rules,
            time: SystemTime::now(),
        });

        false
    }

    /// Removes any sessions that have the ID provided from the
    /// matchmaking queue
    ///
    /// `sid` The session ID to remove
    pub async fn unqueue_session(&self, sid: SessionID) {
        let queue = &mut self.queue.lock().await;
        queue.retain(|value| value.player.session_id != sid);
    }

    pub async fn modify_game(&self, game_id: GameID, action: GameModifyAction) {
        let games = self.games.read().await;
        if let Some(game) = games.get(&game_id) {
            game.modify(action).await;
        }
    }

    pub async fn remove_player(&self, game_id: GameID, ty: RemovePlayerType) {
        let games = self.games.read().await;
        if let Some(game) = games.get(&game_id) {
            let is_empty = game.remove_player(ty).await;
            if is_empty {
                drop(games);

                // Remove the empty game
                let games = &mut *self.games.write().await;
                games.remove(&game_id);
            }
        }
    }
}
