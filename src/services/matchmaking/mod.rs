use crate::{
    services::game::{AddPlayerMessage, CheckJoinableMessage, Game, GameJoinableState, GamePlayer},
    utils::types::{GameID, SessionID},
};
use interlink::prelude::*;
use log::debug;
use rules::RuleSet;
use std::{collections::VecDeque, sync::Arc, time::SystemTime};

pub mod rules;

#[derive(Service)]
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
#[derive(Message)]
pub struct GameCreatedMessage {
    /// The link to the game
    pub link: Link<Game>,
    /// The ID of the created game
    pub game_id: GameID,
}

impl Handler<GameCreatedMessage> for Matchmaking {
    type Response = Sfr<Self, GameCreatedMessage>;

    fn handle(
        &mut self,
        msg: GameCreatedMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        Sfr::new(move |service: &mut Matchmaking, _ctx| {
            Box::pin(async move {
                let link = msg.link;
                let queue = &mut service.queue;
                if queue.is_empty() {
                    return;
                }

                let mut requeue = VecDeque::new();

                while let Some(entry) = queue.pop_front() {
                    let join_state = match link
                        .send(CheckJoinableMessage {
                            rule_set: Some(entry.rule_set.clone()),
                        })
                        .await
                    {
                        Ok(value) => value,
                        // Game is no longer available
                        Err(_) => {
                            requeue.push_back(entry);
                            break;
                        }
                    };

                    match join_state {
                        GameJoinableState::Joinable => {
                            debug!(
                                "Found player from queue adding them to the game (GID: {})",
                                msg.game_id
                            );
                            let time = SystemTime::now();
                            let elapsed = time.duration_since(entry.time);
                            if let Ok(elapsed) = elapsed {
                                debug!("Matchmaking time elapsed: {}s", elapsed.as_secs())
                            }

                            // Add the player to the game
                            if link
                                .do_send(AddPlayerMessage {
                                    player: entry.player,
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                        GameJoinableState::Full => {
                            // If the game is not joinable push the entry back to the
                            // front of the queue and early return
                            requeue.push_back(entry);
                            break;
                        }
                        GameJoinableState::NotMatch => {
                            // TODO: Check started time and timeout
                            // player if they've been waiting too long
                            requeue.push_back(entry);
                        }
                    }
                }

                queue.append(&mut requeue)
            })
        })
    }
}

/// Message to add a new player to the matchmaking queue
#[derive(Message)]
pub struct QueuePlayerMessage {
    /// The player to add to the queue
    pub player: GamePlayer,
    /// The rules for the player
    pub rule_set: Arc<RuleSet>,
}

impl Handler<QueuePlayerMessage> for Matchmaking {
    /// Empty response type
    type Response = ();

    fn handle(&mut self, msg: QueuePlayerMessage, _ctx: &mut ServiceContext<Self>) {
        let time = SystemTime::now();
        self.queue.push_back(QueueEntry {
            player: msg.player,
            rule_set: msg.rule_set,
            time,
        });
    }
}

/// Message to remove a player from the matchmaking queue
#[derive(Message)]
pub struct RemoveQueueMessage {
    /// The session ID of the player to remove
    pub session_id: SessionID,
}

impl Handler<RemoveQueueMessage> for Matchmaking {
    /// Empty response type
    type Response = ();

    fn handle(&mut self, msg: RemoveQueueMessage, _ctx: &mut ServiceContext<Self>) {
        self.queue
            .retain(|value| value.player.session_id != msg.session_id);
    }
}
