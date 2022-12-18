use super::{
    player::GamePlayer, rules::RuleSet, Game, GameJoinableState, GameModifyAction, GameSnapshot,
    RemovePlayerType,
};
use crate::utils::types::{GameID, SessionID};
use blaze_pk::types::TdfMap;
use log::debug;
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::SystemTime,
};
use tokio::sync::{oneshot, Mutex, RwLock};

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
    rules: Arc<RuleSet>,
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
            .values()
            .map(|game| async {
                let (sender, reciever) = oneshot::channel();
                game.handle_action(GameModifyAction::Snapshot(sender)).await;
                reciever.await.ok()
            })
            .collect::<Vec<_>>();
        futures_util::future::join_all(snapshots)
            .await
            .into_iter()
            .filter_map(|value| value)
            .collect()
    }

    /// Takes a snapshot of the game with the provided game ID
    ///
    /// `game_id` The ID of the game to take the snapshot of
    pub async fn snapshot_id(&self, game_id: GameID) -> Option<GameSnapshot> {
        let games = &*self.games.read().await;
        let game = games.get(&game_id)?;

        let (sender, reciever) = oneshot::channel();
        game.handle_action(GameModifyAction::Snapshot(sender)).await;
        reciever.await.ok()
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
        game.handle_action(GameModifyAction::AddPlayer(player))
            .await;
        self.update_queue(game).await;
    }

    /// Updates the matchmaking queue for the provided game. Will look through
    /// the queue checking if the player rules match the game attributes and if
    /// they do then add them to the game.
    ///
    /// `game` The game to update to queue with
    async fn update_queue(&self, game: &Game) {
        let queue = &mut *self.queue.lock().await;
        if !queue.is_empty() {
            let mut unmatched = VecDeque::new();
            while let Some(entry) = queue.pop_front() {
                let (sender, reciever) = oneshot::channel();
                game.handle_action(GameModifyAction::CheckJoinable(
                    Some(entry.rules.clone()),
                    sender,
                ))
                .await;
                let join_state = reciever.await.unwrap_or(GameJoinableState::Full);
                match join_state {
                    GameJoinableState::Full => {
                        // If the game is not joinable push the entry back to the
                        // front of the queue and early return
                        queue.push_front(entry);
                        return;
                    }
                    GameJoinableState::NotMatch => {
                        // TODO: Check started time and timeout
                        // player if they've been waiting too long
                        unmatched.push_back(entry);
                    }
                    GameJoinableState::Joinable => {
                        debug!(
                            "Found player from queue adding them to the game (GID: {})",
                            game.id
                        );
                        let time = SystemTime::now();
                        let elapsed = time.duration_since(entry.time);
                        if let Ok(elapsed) = elapsed {
                            debug!("Matchmaking time elapsed: {}s", elapsed.as_secs())
                        }
                        game.handle_action(GameModifyAction::AddPlayer(entry.player))
                            .await;
                    }
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
        let rules = Arc::new(rules);
        let games = &*self.games.read().await;
        for game in games.values() {
            let (sender, reciever) = oneshot::channel();
            game.handle_action(GameModifyAction::CheckJoinable(Some(rules.clone()), sender))
                .await;
            let join_state = reciever.await.unwrap_or(GameJoinableState::Full);
            match join_state {
                GameJoinableState::Joinable => {
                    debug!("Found matching game (GID: {})", game.id);
                    game.handle_action(GameModifyAction::AddPlayer(player))
                        .await;
                    return true;
                }
                _ => {}
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
            game.handle_action(action).await;
        }
    }

    pub async fn remove_player(&self, game_id: GameID, ty: RemovePlayerType) {
        let games = self.games.read().await;
        if let Some(game) = games.get(&game_id) {
            let (sender, reciever) = oneshot::channel();
            game.handle_action(GameModifyAction::RemovePlayer(ty, sender))
                .await;
            let is_empty = reciever.await.unwrap_or(true);
            if is_empty {
                drop(games);

                // Remove the empty game
                let games = &mut *self.games.write().await;
                games.remove(&game_id);
            }
        }
    }
}
