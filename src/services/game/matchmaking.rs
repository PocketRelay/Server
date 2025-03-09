use std::{collections::VecDeque, time::SystemTime};

use log::debug;
use parking_lot::Mutex;

use crate::{
    config::Config,
    services::tunnel::TunnelService,
    session::{
        models::game_manager::{AsyncMatchmakingStatus, GameSetupContext, MatchmakingResult},
        packet::Packet,
    },
    utils::{
        components::game_manager,
        types::{GameID, PlayerID},
    },
};

use super::{rules::RuleSet, GameAddPlayerExt, GameJoinableState, GamePlayer, GameRef};

#[derive(Default)]
pub struct Matchmaking {
    /// Matchmaking entry queue
    queue: Mutex<VecDeque<MatchmakingEntry>>,
}

/// Entry into the matchmaking queue
struct MatchmakingEntry {
    /// The player entry
    player: GamePlayer,
    /// The rules that a game must match for the player to join
    rule_set: RuleSet,
    /// Time that the player entered matchmaking
    started: SystemTime,
}

const DEFAULT_FIT: u16 = 21600;

impl Matchmaking {
    pub fn remove(&self, player_id: PlayerID) {
        self.queue
            .lock()
            .retain(|value| value.player.player.id != player_id);
    }

    pub fn queue(&self, player: GamePlayer, rule_set: RuleSet) {
        let started = SystemTime::now();
        self.queue.lock().push_back(MatchmakingEntry {
            player,
            rule_set,
            started,
        });
    }

    pub fn process_queue(
        &self,
        tunnel_service: &TunnelService,
        config: &Config,

        link: &GameRef,
        game_id: GameID,
    ) {
        let queue = &mut *self.queue.lock();

        while let Some(entry) = queue.front() {
            let join_state = { link.read().joinable_state(Some(&entry.rule_set)) };

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
                    self.add_from_matchmaking(tunnel_service, config, link.clone(), entry.player);
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

    pub fn add_from_matchmaking(
        &self,
        tunnel_service: &TunnelService,
        config: &Config,

        game_ref: GameRef,
        player: GamePlayer,
    ) {
        let session = match player.link.upgrade() {
            Some(value) => value,
            // Session was dropped
            None => return,
        };

        let msid = player.player.id;

        // MUST be sent to players at least once when matchmaking otherwise it may fail
        player.notify(Packet::notify(
            game_manager::COMPONENT,
            game_manager::MATCHMAKING_ASYNC_STATUS,
            AsyncMatchmakingStatus { player_id: msid },
        ));

        // Add player to the game
        game_ref.add_player(
            tunnel_service,
            config,
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
}
