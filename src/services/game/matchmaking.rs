use super::{player::GamePlayer, rules::RuleSet, GameAddr, GameJoinableState};
use crate::utils::types::SessionID;
use log::{debug, error};
use std::{collections::VecDeque, sync::Arc, time::SystemTime};
use tokio::sync::mpsc;

struct Matchmaking {
    /// The queue of matchmaking entries
    queue: VecDeque<QueueEntry>,
    /// Receiver for handling messages from MatchmakingAddr instances
    rx: mpsc::UnboundedReceiver<Message>,
}

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
pub struct MatchmakingAddr(mpsc::UnboundedSender<Message>);

impl MatchmakingAddr {
    pub fn spawn() -> MatchmakingAddr {
        let (tx, rx) = mpsc::unbounded_channel();
        let this = Matchmaking {
            queue: Default::default(),
            rx,
        };
        let addr = MatchmakingAddr(tx);
        tokio::spawn(this.process());
        addr
    }

    /// Attempts to remove the player with the provided Session ID from
    /// the matchmaking queue
    ///
    /// `id` The Session ID of the player to remove
    pub fn unqueue_session(&self, id: SessionID) {
        if let Err(_) = self.0.send(Message::RemovePlayer(id)) {
            error!("Failed to remove player from matchmaking queue: {}", id);
        }
    }

    /// Handles the creation of a new game
    ///
    /// `game` The addr to the created game
    pub fn created(&self, game: GameAddr) {
        if let Err(_) = self.0.send(Message::GameCreated(game)) {
            error!("Failed to handle game creation");
        }
    }

    /// Handles the creation of a new game
    ///
    /// `player`   The player to add to the queue
    /// `rule_set` The player rule set
    pub fn queue(&self, player: GamePlayer, rule_set: Arc<RuleSet>) {
        if let Err(_) = self.0.send(Message::QueuePlayer(player, rule_set)) {
            error!("Failed to queue player");
        }
    }
}

enum Message {
    /// Message recieved when a new game is created used to
    /// check against any players waiting in the queue to
    /// see if they are able to join
    GameCreated(GameAddr),

    /// Queues the player in the matchmaking queue to list
    /// for future game creation events
    QueuePlayer(GamePlayer, Arc<RuleSet>),

    /// Removes any players with the provided session ID
    /// from the matchmaking queue
    RemovePlayer(SessionID),
}

impl Matchmaking {
    /// Handling function for processing incoming messages from
    /// the matchmaking addr instances
    pub async fn process(mut self) {
        while let Some(message) = self.rx.recv().await {
            match message {
                Message::GameCreated(addr) => self.handle_game_created(addr).await,
                Message::QueuePlayer(player, rules) => self.handle_queue_player(player, rules),
                Message::RemovePlayer(id) => self.handle_remove_player(id),
            }
        }
    }

    async fn handle_game_created(&mut self, addr: GameAddr) {
        let queue = &mut self.queue;
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
    }

    fn handle_queue_player(&mut self, player: GamePlayer, rules: Arc<RuleSet>) {
        let time = SystemTime::now();
        self.queue.push_back(QueueEntry {
            player,
            rules,
            time,
        })
    }

    fn handle_remove_player(&mut self, id: SessionID) {
        self.queue.retain(|value| value.player.addr.id != id);
    }
}
