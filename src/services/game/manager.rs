use super::{
    models::{DatalessContext, GameSettings, GameSetupContext, PlayerState},
    rules::RuleSet,
    AddPlayerMessage, AttrMap, CheckJoinableMessage, Game, GameJoinableState, GamePlayer,
    GameSnapshot,
};
use crate::{
    services::game::models::AsyncMatchmakingStatus,
    session::{packet::Packet, PushExt},
    utils::{
        components::game_manager,
        hashing::IntHashMap,
        types::{GameID, PlayerID},
    },
};
use interlink::prelude::*;
use log::debug;
use std::{
    collections::VecDeque,
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
    games: RwLock<IntHashMap<GameID, Link<Game>>>,
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
            let mut items: Vec<(&GameID, &Link<Game>)> = games.iter().collect();
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
                        game.send(super::SnapshotMessage { include_net }).await
                    });
                });

            more
        };

        // Allocate a list for the snapshots
        let mut snapshots = Vec::with_capacity(join_set.len());

        // Recieve all the snapshots from their tasks
        while let Some(result) = join_set.join_next().await {
            if let Ok(Ok(snapshot)) = result {
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
        attributes: AttrMap,
        setting: GameSettings,
        mut host: GamePlayer,
    ) -> (Link<Game>, GameID) {
        let id = self.next_id.fetch_add(1, Ordering::AcqRel);

        host.state = PlayerState::ActiveConnected;

        let link = Game::start(id, attributes, setting, self.clone());
        {
            let games = &mut *self.games.write().await;
            games.insert(id, link.clone());
        }

        let _ = link.do_send(AddPlayerMessage {
            player: host,
            context: GameSetupContext::Dataless(DatalessContext::CreateGameSetup),
        });

        (link, id)
    }

    pub async fn get_game(&self, game_id: GameID) -> Option<Link<Game>> {
        let games = &*self.games.read().await;
        games.get(&game_id).cloned()
    }

    pub async fn try_add(
        &self,
        player: GamePlayer,
        rule_set: Arc<RuleSet>,
    ) -> Result<(), GamePlayer> {
        let games = &*self.games.read().await;

        // Message asking for the game joinable state
        let msg = CheckJoinableMessage {
            rule_set: Some(rule_set),
        };

        // Attempt to find a game thats joinable
        for (id, link) in games {
            // Check if the game is joinable
            if let Ok(GameJoinableState::Joinable) = link.send(msg.clone()).await {
                debug!("Found matching game (GID: {})", id);
                let msid = player.player.id;
                let _ = link.do_send(AddPlayerMessage {
                    player,
                    context: GameSetupContext::Matchmaking(msid),
                });
                return Ok(());
            }
        }

        Err(player)
    }

    pub async fn remove_game(&self, game_id: GameID) {
        let games = &mut *self.games.write().await;
        if let Some(game) = games.remove(&game_id) {
            game.stop();
        }
    }

    pub async fn process_queue(&self, link: Link<Game>, game_id: GameID) {
        let queue = &mut *self.queue.write().await;
        if queue.is_empty() {
            return;
        }

        while let Some(entry) = queue.front() {
            let join_state = match link
                .send(CheckJoinableMessage {
                    rule_set: Some(entry.rule_set.clone()),
                })
                .await
            {
                Ok(value) => value,
                // Game is no longer available stop checking
                Err(_) => break,
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

                    // Add the player to the game
                    if link
                        .do_send(AddPlayerMessage {
                            player: entry.player,
                            context: GameSetupContext::Matchmaking(msid),
                        })
                        .is_err()
                    {
                        break;
                    }
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
