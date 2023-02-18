use super::{player::GamePlayer, rules::RuleSet, GameAddr, GameJoinableState};
use crate::utils::types::SessionID;
use interlink::{msg::ServiceFutureResponse, prelude::*};
use log::debug;
use std::{collections::VecDeque, sync::Arc, time::SystemTime};

struct Matchmaking {
    /// The queue of matchmaking entries
    queue: VecDeque<QueueEntry>,
}

impl Service for Matchmaking {}

/// Structure of a entry within the matchmaking queue
/// containing information about the queue item
struct QueueEntry {
    /// The player this entry is for
    player: GamePlayer,
    /// The set of rules the game must match in
    /// order for this player to be removed from
    /// the queue and placed into a game
    rules: Arc<RuleSet>,
    /// The system time of when the player was added
    /// to the matchmaking queue
    time: SystemTime,
}

#[derive(Clone)]
pub struct MatchmakingLink(Link<Matchmaking>);

impl MatchmakingLink {
    pub fn start() -> MatchmakingLink {
        let this = Matchmaking {
            queue: Default::default(),
        };
        let link = this.start();
        MatchmakingLink(link)
    }

    /// Attempts to remove the player with the provided Session ID from
    /// the matchmaking queue
    ///
    /// `id` The Session ID of the player to remove
    pub fn unqueue_session(&self, id: SessionID) {
        self.0.do_send(RemovePlayer { session_id: id }).ok();
    }

    /// Handles the creation of a new game
    ///
    /// `game` The addr to the created game
    pub fn created(&self, game: GameAddr) {
        self.0.do_send(GameCreated { addr: game }).ok();
    }

    /// Handles the creation of a new game
    ///
    /// `player`   The player to add to the queue
    /// `rule_set` The player rule set
    pub fn queue(&self, player: GamePlayer, rule_set: Arc<RuleSet>) {
        self.0
            .do_send(QueuePlayer {
                player,
                rules: rule_set,
            })
            .ok();
    }
}

struct GameCreated {
    addr: GameAddr,
}

impl Message for GameCreated {
    type Response = ();
}

impl Handler<GameCreated> for Matchmaking {
    type Response = ServiceFutureResponse<Self, GameCreated>;

    fn handle(&mut self, msg: GameCreated, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        ServiceFutureResponse::new(move |service: &mut Matchmaking, _ctx| {
            Box::pin(async move {
                let addr = msg.addr;
                let queue = &mut service.queue;
                if queue.is_empty() {
                    return;
                }

                let checking_queue = queue.split_off(0);
                for entry in checking_queue {
                    let join_state = addr.check_joinable(entry.rules.clone()).await;
                    match join_state {
                        GameJoinableState::Joinable => {
                            debug!(
                                "Found player from queue adding them to the game (GID: {})",
                                addr.id
                            );
                            let time = SystemTime::now();
                            let elapsed = time.duration_since(entry.time);
                            if let Ok(elapsed) = elapsed {
                                debug!("Matchmaking time elapsed: {}s", elapsed.as_secs())
                            }
                            addr.add_player(entry.player);
                        }
                        GameJoinableState::Full => {
                            // If the game is not joinable push the entry back to the
                            // front of the queue and early return
                            queue.push_back(entry);
                            return;
                        }
                        GameJoinableState::NotMatch => {
                            // TODO: Check started time and timeout
                            // player if they've been waiting too long
                            queue.push_back(entry);
                        }
                    }
                }
            })
        })
    }
}

struct QueuePlayer {
    player: GamePlayer,
    rules: Arc<RuleSet>,
}

impl Message for QueuePlayer {
    type Response = ();
}

impl Handler<QueuePlayer> for Matchmaking {
    type Response = ();

    fn handle(&mut self, msg: QueuePlayer, _ctx: &mut ServiceContext<Self>) {
        let time = SystemTime::now();
        self.queue.push_back(QueueEntry {
            player: msg.player,
            rules: msg.rules,
            time,
        })
    }
}

struct RemovePlayer {
    session_id: SessionID,
}

impl Message for RemovePlayer {
    type Response = ();
}

impl Handler<RemovePlayer> for Matchmaking {
    type Response = ();

    fn handle(&mut self, msg: RemovePlayer, _ctx: &mut ServiceContext<Self>) {
        self.queue
            .retain(|value| value.player.session_id != msg.session_id);
    }
}
