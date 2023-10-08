use super::{rules::RuleSet, AttrMap, Game, GameJoinableState, GamePlayer, GameRef, GameSnapshot};
use crate::{
    session::{
        models::game_manager::{
            AsyncMatchmakingStatus, GameSettings, GameSetupContext, MatchmakingResult,
        },
        packet::Packet,
    },
    utils::{
        components::game_manager,
        hashing::IntHashMap,
        types::{GameID, PlayerID},
    },
};
use log::{debug, warn};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::{Duration, SystemTime},
};
use tokio::{sync::RwLock, task::JoinSet};

/// Manager which controls all the active games on the server
/// commanding them to do different actions and removing them
/// once they are no longer used
pub struct GameManager {
    /// The map of games to the actual game address
    games: RwLock<IntHashMap<GameID, GameRef>>,
    /// Stored value for the ID to give the next game
    next_id: AtomicU32,
    /// Matchmaking entry queue
    queue: RwLock<VecDeque<MatchmakingEntry>>,
}

/// Entry into the matchmaking queue
struct MatchmakingEntry {
    /// The player entry
    player: GamePlayer,
    /// The rules that a game must match for the player to join
    rule_set: Arc<RuleSet>,
    /// Time that the player entered matchmaking
    started: SystemTime,
}

const DEFAULT_FIT: u16 = 21600;

impl GameManager {
    /// Max number of times to poll a game for shutdown before erroring
    const MAX_RELEASE_ATTEMPTS: u8 = 20;

    /// Starts a new game manager service returning its link
    pub fn new() -> Self {
        Self {
            games: Default::default(),
            next_id: AtomicU32::new(1),
            queue: Default::default(),
        }
    }

    pub async fn create_snapshot(
        &self,
        offset: usize,
        count: usize,
        include_net: bool,
    ) -> (Vec<GameSnapshot>, bool) {
        // Create the futures using the handle action before passing
        // them to a future to be awaited
        let mut join_set = JoinSet::new();

        let more = {
            let games = &*self.games.read().await;

            // Create an ordered set
            let mut items: Vec<(&GameID, &GameRef)> = games.iter().collect();
            items.sort_by_key(|(key, _)| *key);

            // Whether there is more keys that what was requested
            let more = items.len() > offset + count;

            // Spawn tasks for obtaining snapshots to each game
            items
                .into_iter()
                // Skip to the desired offset
                .skip(offset)
                // Take the desired number of keys
                .take(count)
                // Iterate over the game links
                .map(|(_, value)| value.clone())
                // Spawn the snapshot tasks
                .for_each(|game| {
                    join_set.spawn(async move {
                        let game = &*game.read().await;
                        game.snapshot(include_net)
                    });
                });

            more
        };

        // Allocate a list for the snapshots
        let mut snapshots = Vec::with_capacity(join_set.len());

        // Recieve all the snapshots from their tasks
        while let Some(result) = join_set.join_next().await {
            if let Ok(snapshot) = result {
                snapshots.push(snapshot);
            }
        }

        (snapshots, more)
    }

    pub async fn remove_queue(&self, player_id: PlayerID) {
        let queue = &mut *self.queue.write().await;
        queue.retain(|value| value.player.player.id != player_id);
    }

    pub async fn queue(&self, player: GamePlayer, rule_set: Arc<RuleSet>) {
        let started = SystemTime::now();
        let queue = &mut *self.queue.write().await;
        queue.push_back(MatchmakingEntry {
            player,
            rule_set,
            started,
        });
    }

    pub async fn add_to_game(
        &self,
        game_ref: GameRef,
        player: GamePlayer,
        context: GameSetupContext,
    ) {
        let player_link = player.link.clone();

        // Add the player to the game
        let game_id = {
            let game = &mut *game_ref.write().await;
            game.add_player(player, context).await;
            game.id
        };

        // Update the player current game
        player_link.set_game(game_id, game_ref).await;
    }

    pub async fn add_from_matchmaking(&self, game_ref: GameRef, player: GamePlayer) {
        let msid = player.player.id;

        // MUST be sent to players atleast once when matchmaking otherwise it may fail
        player.notify_handle.notify(Packet::notify(
            game_manager::COMPONENT,
            game_manager::MATCHMAKING_ASYNC_STATUS,
            AsyncMatchmakingStatus { player_id: msid },
        ));

        // Add the player to the game
        self.add_to_game(
            game_ref,
            player,
            GameSetupContext::Matchmaking {
                fit_score: DEFAULT_FIT,
                max_fit_score: DEFAULT_FIT,
                session_id: msid,
                result: MatchmakingResult::JoinedExistingGame,
                player_id: msid,
            },
        )
        .await;
    }

    pub async fn create_game(
        self: &Arc<Self>,
        attributes: AttrMap,
        setting: GameSettings,
    ) -> (GameRef, GameID) {
        let id = self.next_id.fetch_add(1, Ordering::AcqRel);
        let game = Game::new(id, attributes, setting, self.clone());
        let link = Arc::new(RwLock::new(game));
        {
            let games = &mut *self.games.write().await;
            games.insert(id, link.clone());
        }

        (link, id)
    }

    pub async fn get_game(&self, game_id: GameID) -> Option<GameRef> {
        let games = &*self.games.read().await;
        games.get(&game_id).cloned()
    }

    pub async fn try_add(&self, player: GamePlayer, rule_set: &RuleSet) -> Result<(), GamePlayer> {
        let games = &*self.games.read().await;

        // Attempt to find a game thats joinable
        for (id, link) in games {
            let join_state = {
                let link = &*link.read().await;
                link.joinable_state(Some(rule_set))
            };

            // Check if the game is joinable
            if let GameJoinableState::Joinable = join_state {
                debug!("Found matching game (GID: {})", id);

                // Add the player to the game
                self.add_from_matchmaking(link.clone(), player).await;

                return Ok(());
            }
        }

        Err(player)
    }

    pub async fn remove_game(&self, game_id: GameID) {
        let games = &mut *self.games.write().await;
        if let Some(mut game) = games.remove(&game_id) {
            let mut attempt: u8 = 1;

            // Attempt to obtain the owned game
            let game = loop {
                if attempt > Self::MAX_RELEASE_ATTEMPTS {
                    let references = Arc::strong_count(&game);
                    warn!(
                        "Failed to stop game {} there are still {} references to it",
                        game_id, references
                    );
                    return;
                }

                match Arc::try_unwrap(game) {
                    Ok(value) => break value,
                    Err(arc) => {
                        let wait = 5 * attempt as u64;
                        let references = Arc::strong_count(&arc);
                        debug!(
                            "Game {} still has {} references to it, waiting {}s",
                            game_id, references, wait
                        );
                        tokio::time::sleep(Duration::from_secs(wait)).await;
                        game = arc;
                        attempt += 1;
                        continue;
                    }
                }
            };

            let game = game.into_inner();
            game.stopped();
        }
    }

    pub async fn process_queue(&self, link: GameRef, game_id: GameID) {
        let queue = &mut *self.queue.write().await;
        if queue.is_empty() {
            return;
        }

        while let Some(entry) = queue.front() {
            let join_state = {
                let link = &*link.read().await;
                link.joinable_state(Some(&entry.rule_set))
            };

            // TODO: If player has been in queue long enough create
            // a game matching their specifics

            match join_state {
                GameJoinableState::Joinable => {
                    let entry = queue
                        .pop_front()
                        .expect("Expecting matchmaking entry but nothing was present");

                    debug!(
                        "Found player from queue adding them to the game (GID: {})",
                        game_id
                    );
                    let time = SystemTime::now();
                    let elapsed = time.duration_since(entry.started);
                    if let Ok(elapsed) = elapsed {
                        debug!("Matchmaking time elapsed: {}s", elapsed.as_secs())
                    }

                    // Add the player to the game
                    self.add_from_matchmaking(link.clone(), entry.player).await;
                }
                GameJoinableState::Full | GameJoinableState::Stopping => {
                    // If the game is not joinable push the entry back to the
                    // front of the queue and early return
                    break;
                }
                GameJoinableState::NotMatch => {
                    // TODO: Check started time and timeout
                    // player if they've been waiting too long
                }
            }
        }
    }
}
