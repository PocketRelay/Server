use super::{player::GamePlayer, rules::RuleSet, GameAddr, GameJoinableState};
use crate::utils::types::SessionID;
use futures::FutureExt;
use interlink::{
    msg::{MessageResponse, ServiceFutureResponse},
    prelude::*,
};
use log::debug;
use std::{collections::VecDeque, sync::Arc, time::SystemTime};

pub struct Matchmaking {
    /// The queue of matchmaking entries
    queue: VecDeque<QueueEntry>,
}

impl Matchmaking {
    /// Starts a new matchmaking service returning its link
    pub fn start() -> Link<Matchmaking> {
        let this = Matchmaking {
            queue: Default::default(),
        };
        this.start()
    }
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
    rule_set: Arc<RuleSet>,
    /// The system time of when the player was added
    /// to the matchmaking queue
    time: SystemTime,
}
/// Message for handling when a game is created and attempting
/// to add players from the queue into the game
pub struct GameCreatedMessage {
    /// The link to the game
    pub link: GameAddr,
}

impl Message for GameCreatedMessage {
    /// Empty response from future
    type Response = ();
}

impl Handler<GameCreatedMessage> for Matchmaking {
    type Response = ServiceFutureResponse<Self, GameCreatedMessage>;

    fn handle(
        &mut self,
        msg: GameCreatedMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        ServiceFutureResponse::new(move |service: &mut Matchmaking, _ctx| {
            async move {
                let addr = msg.link;
                let queue = &mut service.queue;
                if queue.is_empty() {
                    return;
                }

                let checking_queue = queue.split_off(0);
                for entry in checking_queue {
                    let join_state = addr.check_joinable(entry.rule_set.clone()).await;
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
            }
            .boxed()
        })
    }
}

/// Message to add a new player to the matchmaking queue
pub struct QueuePlayerMessage {
    /// The player to add to the queue
    pub player: GamePlayer,
    /// The rules for the player
    pub rule_set: Arc<RuleSet>,
}

impl Message for QueuePlayerMessage {
    /// Empty response type
    type Response = ();
}

impl Handler<QueuePlayerMessage> for Matchmaking {
    /// Empty response type
    type Response = MessageResponse<QueuePlayerMessage>;

    fn handle(
        &mut self,
        msg: QueuePlayerMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        let time = SystemTime::now();
        self.queue.push_back(QueueEntry {
            player: msg.player,
            rule_set: msg.rule_set,
            time,
        });
        MessageResponse(())
    }
}

/// Message to remove a player from the matchmaking queue
pub struct RemoveQueueMessage {
    /// The session ID of the player to remove
    pub session_id: SessionID,
}

impl Message for RemoveQueueMessage {
    /// Empty response type
    type Response = ();
}

impl Handler<RemoveQueueMessage> for Matchmaking {
    /// Empty response type
    type Response = ();

    fn handle(&mut self, msg: RemoveQueueMessage, _ctx: &mut ServiceContext<Self>) {
        self.queue
            .retain(|value| value.player.session_id != msg.session_id);
    }
}
