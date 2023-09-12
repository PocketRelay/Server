use super::{
    models::{DatalessContext, GameSettings, GameSetupContext, PlayerState},
    rules::RuleSet,
    Attributes, Game, GameJoinableState, GamePlayer, GameRef, GameSnapshot,
};
use crate::{
    services::game::models::AsyncMatchmakingStatus,
    session::{packet::Packet, PushExt},
    utils::{
        components::game_manager,
        types::{GameID, PlayerID},
    },
};
use log::debug;
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::SystemTime,
};
use tokio::{sync::RwLock, task::JoinSet};

/// Manager which controls all the active games on the server
/// commanding them to do different actions and removing them
/// once they are no longer used
pub struct GameManager {
    /// The map of games to the actual game address
    games: RwLock<HashMap<GameID, GameRef>>,
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

impl GameManager {
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
                        game.snapshot(include_net).await
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

    pub async fn create_game(
        self: &Arc<Self>,
        attributes: Attributes,
        setting: GameSettings,
        mut host: GamePlayer,
    ) -> GameID {
        let id = self.next_id.fetch_add(1, Ordering::AcqRel);

        host.state = PlayerState::ActiveConnected;

        let game = Game::new(id, setting, attributes, self.clone());
        let game = Arc::new(RwLock::new(game));

        {
            let games = &mut *self.games.write().await;
            games.insert(id, game.clone());
        }

        tokio::spawn(async move {
            let game = &mut *game.write().await;
            game.add_player(
                host,
                GameSetupContext::Dataless(DatalessContext::CreateGameSetup),
            )
            .await;
        });

        let game_manager = self.clone();

        // Notify matchmaking of the new game
        tokio::spawn(async move {
            game_manager.process_queue(id).await;
        });

        id
    }

    pub async fn get_game(&self, game_id: GameID) -> Option<GameRef> {
        let games = &*self.games.read().await;
        games.get(&game_id).cloned()
    }

    pub async fn try_add(
        &self,
        player: GamePlayer,
        rule_set: Arc<RuleSet>,
    ) -> Result<(), GamePlayer> {
        let games = &*self.games.read().await;

        // Attempt to find a game thats joinable
        for (id, game_ref) in games {
            let game = &*game_ref.read().await;

            // Check if the game is joinable
            if let GameJoinableState::Joinable = game.check_joinable(Some(rule_set.clone())).await {
                debug!("Found matching game (GID: {})", id);
                let msid = player.player.id;

                let game = game_ref.clone();

                tokio::spawn(async move {
                    let game = &mut *game.write().await;
                    game.add_player(player, GameSetupContext::Matchmaking(msid))
                        .await;
                });

                return Ok(());
            }
        }

        Err(player)
    }

    pub async fn remove_game(&self, game_id: GameID) {
        let games = &mut *self.games.write().await;
        if let Some(game) = games.remove(&game_id) {
            tokio::spawn(async move {
                let game = &mut *game.write().await;
                game.on_stopped().await;
            });
        }
    }

    pub async fn process_queue(&self, game_id: GameID) {
        let game_ref = match self.get_game(game_id).await {
            Some(value) => value,
            // Game doesn't exist anymore
            None => return,
        };

        let queue = &mut *self.queue.write().await;
        if queue.is_empty() {
            return;
        }

        while let Some(entry) = queue.front() {
            let join_state = {
                let game = &*game_ref.read().await;
                game.check_joinable(Some(entry.rule_set.clone())).await
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

                    let msid = entry.player.player.id;

                    // Send the async update (TODO: Do this at intervals)
                    entry.player.link.push(Packet::notify(
                        game_manager::COMPONENT,
                        game_manager::MATCHMAKING_ASYNC_STATUS,
                        AsyncMatchmakingStatus { player_id: msid },
                    ));

                    let game = &mut *game_ref.write().await;
                    // Add the player to the game
                    game.add_player(entry.player, GameSetupContext::Matchmaking(msid))
                        .await;
                }
                GameJoinableState::Full => {
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
