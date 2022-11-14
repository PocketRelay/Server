use std::{
    collections::{HashMap, VecDeque},
    sync::atomic::{AtomicU32, Ordering},
    time::SystemTime,
};

use blaze_pk::types::TdfMap;
use log::debug;
use tokio::sync::{Mutex, RwLock};

use crate::{blaze::session::SessionArc, game::rules::RuleSet};

use super::game::{AttrMap, Game};

/// Structure for managing games and the matchmaking queue
pub struct GameManager {
    /// Map of Game IDs to the actual games.
    games: RwLock<HashMap<u32, Game>>,
    /// Queue of players wanting to join games
    queue: Mutex<VecDeque<QueueEntry>>,
    /// ID for the next game to create
    id: AtomicU32,
}

/// Structure for a entry in the matchmaking queue
struct QueueEntry {
    /// The session that is waiting in the queue
    session: SessionArc,
    /// The rules that games must meet for this
    /// queue entry to join.
    rules: RuleSet,
    /// The time that the queue entry was created at
    time: SystemTime,
}

impl GameManager {
    /// Creates a new instance of the game manager
    pub fn new() -> Self {
        Self {
            games: RwLock::new(HashMap::new()),
            queue: RwLock::new(VecDeque::new()),
            id: AtomicU32::new(1),
        }
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

    /// Attempts to add the host session to the game with the provided game
    /// ID. If the host is successfully added then the game will be compared
    /// against any players waiting in the matchmaking queue.
    ///
    /// `game_id` The ID of the game to add the session to
    /// `session` The session to add as the host
    pub async fn try_add_host(&self, game_id: u32, session: &SessionArc) {
        let games = &*self.games.read().await;
        let Some(game) = games.get(&game_id) else { return; };
        game.add_player(session).await;
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
                    game.add_player(&entry.session).await;
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
    pub async fn add_or_queue(&self, session: &SessionArc, rules: RuleSet) -> bool {
        let games = &*self.games.read().await;
        for game in games.values() {
            if !game.is_joinable().await {
                continue;
            }
            let game_data = game.data.read().await;
            if rules.matches(&game_data.attributes) {
                debug!("Found matching game (GID: {})", game.id);
                game.add_player(session).await;
                return true;
            }
        }

        let queue = &mut self.queue.lock().await;
        queue.push_back(QueueEntry {
            session: session.clone(),
            rules,
            time: SystemTime::now(),
        });

        false
    }

    /// Removes any sessions that have the ID provided from the
    /// matchmaking queue
    ///
    /// `sid` The session ID to remove
    pub async fn unqueue_session(&self, sid: u32) {
        let queue = &mut self.queue.lock().await;
        queue.retain(|value| value.session.id != sid);
    }

    /// Updates the mesh connection in the game with the provied game
    /// ID for the provied session with the provided target
    ///
    /// `game_id` The ID of the game to update the mesh connection in
    /// `session` The session to update the connection for
    /// `target`  The mesh connection update target
    pub async fn update_mesh_connection(
        &self,
        game_id: u32,
        session: &SessionArc,
        target: u32,
    ) -> bool {
        let games = self.games.read().await;
        let Some(game) = games.get(&game_id) else { return false; };
        game.update_mesh_connection(session, target).await;
        true
    }

    /// Updates the state of the game with the provided id with
    /// the provided state
    ///
    /// `game_id` The ID of the game to update the state of
    /// `state`   The new game state
    pub async fn set_game_state(&self, game_id: u32, state: u16) -> bool {
        let games = self.games.read().await;
        let Some(game) = games.get(&game_id) else { return false; };
        game.set_state(state).await;
        true
    }

    /// Updates the game setting of the game with the provided id with the
    /// provided setting value
    ///
    /// `game_id` The ID of the game to update the setting of
    /// `setting` The new setting value
    pub async fn set_game_setting(&self, game_id: u32, setting: u16) -> bool {
        let games = self.games.read().await;
        let Some(game) = games.get(&game_id) else { return false; };
        game.set_setting(setting).await;
        true
    }

    /// Updates the attributes of the game with the provided id with the
    /// provided attributes map value
    ///
    /// `game_id` The ID of the game to update the setting of
    /// `attributes` The new attributes value
    pub async fn set_game_attributes(&self, game_id: u32, attributes: AttrMap) -> bool {
        let games = self.games.read().await;
        let Some(game) = games.get(&game_id) else { return false; };
        game.set_attributes(attributes).await;
        true
    }

    /// Removes a player by their player ID from the game with the provided
    /// game ID. Will remove the game itself if the game is empty after the
    /// player has been removed.
    ///
    /// `game_id` The game to remove the player from
    /// `pid`     The id of the player to remove
    pub async fn remove_player_pid(&self, game_id: u32, pid: u32) -> bool {
        {
            let games = self.games.read().await;
            let Some(game) = games.get(&game_id) else { return false; };
            game.remove_by_pid(pid).await;
            if game.is_empty().await {
                game.release().await;
            } else {
                return true;
            }
        }
        self.remove_game(game_id).await;
        true
    }

    /// Removes a player by their session ID from the game with the provided
    /// game ID. Will remove the game itself if the game is empty after the
    /// player has been removed.
    ///
    /// `game_id` The game to remove the player from
    /// `sid`     The session id of the player to remove
    pub async fn remove_player_sid(&self, game_id: u32, sid: u32) -> bool {
        {
            let games = self.games.read().await;
            let Some(game) = games.get(&game_id) else { return false; };
            game.remove_by_sid(sid).await;
            if game.is_empty().await {
                game.release().await;
            } else {
                return true;
            }
        }
        self.remove_game(game_id).await;
        true
    }

    /// Removes any games with the provided game id
    ///
    /// `game_id` The ID of the game to remove
    async fn remove_game(&self, game_id: u32) {
        let games = &mut *self.games.write().await;
        games.remove(&game_id);
    }
}
