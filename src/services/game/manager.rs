use super::{rules::RuleSet, AttrMap, Game, GameJoinableState, GamePlayer, GameRef, GameSnapshot};
use crate::{
    config::RuntimeConfig,
    services::tunnel::TunnelService,
    session::{
        models::game_manager::{
            AsyncMatchmakingStatus, GameSettings, GameSetupContext, MatchmakingResult,
        },
        packet::Packet,
        SessionLink,
    },
    utils::{
        components::game_manager,
        hashing::IntHashMap,
        types::{GameID, PlayerID},
    },
};
use chrono::Utc;
use log::debug;
use parking_lot::{Mutex, RwLock};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::SystemTime,
};

/// Manager which controls all the active games on the server
/// commanding them to do different actions and removing them
/// once they are no longer used
pub struct GameManager {
    /// The map of games to the actual game address
    games: RwLock<IntHashMap<GameID, GameRef>>,
    /// Stored value for the ID to give the next game
    next_id: AtomicU32,
    /// Matchmaking entry queue
    queue: Mutex<VecDeque<MatchmakingEntry>>,
    /// Tunneling service
    tunnel_service: Arc<TunnelService>,
    /// Runtime configuration
    config: Arc<RuntimeConfig>,
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
    /// Starts a new game manager service returning its link
    pub fn new(tunnel_service: Arc<TunnelService>, config: Arc<RuntimeConfig>) -> Self {
        Self {
            games: Default::default(),
            next_id: AtomicU32::new(1),
            queue: Default::default(),
            tunnel_service,
            config,
        }
    }

    /// Obtains the total count of games in the list
    pub fn get_total_games(&self) -> usize {
        let games = &*self.games.read();
        games.len()
    }

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
            .map(|game| {
                let game = &*game.read();
                game.snapshot(include_net, include_players)
            })
            .collect();

        (snapshots, more)
    }

    pub fn remove_queue(&self, player_id: PlayerID) {
        let queue = &mut *self.queue.lock();
        queue.retain(|value| value.player.player.id != player_id);
    }

    pub fn queue(&self, player: GamePlayer, rule_set: Arc<RuleSet>) {
        let started = SystemTime::now();
        let queue = &mut *self.queue.lock();
        queue.push_back(MatchmakingEntry {
            player,
            rule_set,
            started,
        });
    }

    pub fn add_to_game(
        &self,
        game_ref: GameRef,
        player: GamePlayer,
        session: SessionLink,
        context: GameSetupContext,
    ) {
        // Add the player to the game
        let (game_id, index) = {
            let game = &mut *game_ref.write();
            let slot = game.add_player(player, context, &self.config);
            (game.id, slot)
        };

        // Allocate tunnel if supported by client
        if let Some(association) = session.data.get_association() {
            self.tunnel_service
                .associate_pool(association, game_id, index as u8);
        }

        // Update the player current game
        session.data.set_game(game_id, Arc::downgrade(&game_ref));
    }

    pub fn add_from_matchmaking(&self, game_ref: GameRef, player: GamePlayer) {
        let session = match player.link.upgrade() {
            Some(value) => value,
            // Session was dropped
            None => return,
        };

        let msid = player.player.id;

        // MUST be sent to players at least once when matchmaking otherwise it may fail
        player.notify_handle.notify(Packet::notify(
            game_manager::COMPONENT,
            game_manager::MATCHMAKING_ASYNC_STATUS,
            AsyncMatchmakingStatus { player_id: msid },
        ));

        // Add the player to the game
        self.add_to_game(
            game_ref,
            player,
            session,
            GameSetupContext::Matchmaking {
                fit_score: DEFAULT_FIT,
                max_fit_score: DEFAULT_FIT,
                session_id: msid,
                result: MatchmakingResult::JoinedExistingGame,
                player_id: msid,
            },
        );
    }

    pub fn create_game(
        self: &Arc<Self>,
        attributes: AttrMap,
        setting: GameSettings,
    ) -> (GameRef, GameID) {
        let id = self.next_id.fetch_add(1, Ordering::AcqRel);
        let created_at = Utc::now();
        let game = Game::new(
            id,
            attributes,
            setting,
            created_at,
            self.clone(),
            self.tunnel_service.clone(),
        );
        let link = Arc::new(parking_lot::RwLock::new(game));
        {
            let games = &mut *self.games.write();
            games.insert(id, link.clone());
        }

        (link, id)
    }

    pub fn get_game(&self, game_id: GameID) -> Option<GameRef> {
        let games = &*self.games.read();
        games.get(&game_id).cloned()
    }

    pub fn try_add(&self, player: GamePlayer, rule_set: &RuleSet) -> Result<(), GamePlayer> {
        let games = &*self.games.read();

        // Attempt to find a game thats joinable
        for (id, link) in games {
            let join_state = {
                let link = &*link.read();
                link.joinable_state(Some(rule_set))
            };

            // Check if the game is joinable
            if let GameJoinableState::Joinable = join_state {
                debug!("Found matching game (GID: {})", id);

                // Add the player to the game
                self.add_from_matchmaking(link.clone(), player);

                return Ok(());
            }
        }

        Err(player)
    }

    pub fn remove_game(&self, game_id: GameID) {
        let games = &mut *self.games.write();
        _ = games.remove(&game_id);
    }

    pub fn process_queue(&self, link: GameRef, game_id: GameID) {
        let queue = &mut *self.queue.lock();
        if queue.is_empty() {
            return;
        }

        while let Some(entry) = queue.front() {
            let join_state = {
                let link = &*link.read();
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
                    self.add_from_matchmaking(link.clone(), entry.player);
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
