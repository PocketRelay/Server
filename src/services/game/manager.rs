use super::{
    player::GamePlayer, rules::RuleSet, AttrMap, GameAddr, GameJoinableState, GameModifyAction,
    GameSnapshot, RemovePlayerType,
};
use crate::utils::types::GameID;
use log::debug;
use std::{collections::HashMap, sync::Arc};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinSet,
};

/// Manager which controls all the active games on the server
/// commanding them to do different actions and removing them
/// once they are no longer used
struct GameManager {
    /// The map of games to the actual game address
    games: HashMap<GameID, GameAddr>,
    /// Stored value for the ID to give the next game
    next_id: u32,
    /// Receiver for handling messages from GameManagerAddr instances
    rx: mpsc::UnboundedReceiver<Message>,
}

#[derive(Clone)]
pub struct GameManagerAddr(mpsc::UnboundedSender<Message>);

enum Message {
    /// Message for taking a snapshot of multiple games at
    /// a specific offset
    SnapshotQuery {
        offset: usize,
        count: usize,
        tx: oneshot::Sender<(Vec<GameSnapshot>, bool)>,
    },

    /// Message for taking a snapshot of a game with a specific
    /// game ID
    Snapshot {
        game_id: GameID,
        tx: oneshot::Sender<Option<GameSnapshot>>,
    },

    /// Message for creating a new game
    Create {
        attributes: AttrMap,
        setting: u16,
        host: GamePlayer,
        tx: oneshot::Sender<GameAddr>,
    },

    /// Attempts to add the provided player to any existing
    /// games that match the player rule set provided
    TryAdd(GamePlayer, Arc<RuleSet>, oneshot::Sender<TryAddResult>),

    /// Message recieved telling the game manager to pass
    /// on a modification action to the game with the
    /// provided Game ID
    ModifyGame(GameID, GameModifyAction),

    /// Message to remove a player from the game with
    /// the provided Game ID
    RemovePlayer(GameID, RemovePlayerType),
}

impl GameManagerAddr {
    pub fn spawn() -> GameManagerAddr {
        let (tx, rx) = mpsc::unbounded_channel();
        let this = GameManager {
            games: Default::default(),
            next_id: 1,
            rx,
        };
        let addr = GameManagerAddr(tx);
        tokio::spawn(this.process());
        addr
    }

    pub async fn create(
        &self,
        attributes: AttrMap,
        setting: u16,
        host: GamePlayer,
    ) -> Option<GameAddr> {
        let (tx, rx) = oneshot::channel();

        if let Err(_) = self.0.send(Message::Create {
            attributes,
            setting,
            host,
            tx,
        }) {
            return None;
        }

        rx.await.ok()
    }

    pub async fn try_add(
        &self,
        player: GamePlayer,
        rule_set: Arc<RuleSet>,
    ) -> Option<TryAddResult> {
        let (tx, rx) = oneshot::channel();

        if let Err(_) = self.0.send(Message::TryAdd(player, rule_set, tx)) {
            return None;
        }

        rx.await.ok()
    }

    pub async fn snapshot(&self, game_id: GameID) -> Option<GameSnapshot> {
        let (tx, rx) = oneshot::channel();

        if let Err(_) = self.0.send(Message::Snapshot { game_id, tx }) {
            return None;
        }

        rx.await.ok().flatten()
    }

    pub async fn snapshot_query(
        &self,
        offset: usize,
        count: usize,
    ) -> Option<(Vec<GameSnapshot>, bool)> {
        let (tx, rx) = oneshot::channel();

        if let Err(_) = self.0.send(Message::SnapshotQuery { offset, count, tx }) {
            return None;
        }

        rx.await.ok()
    }

    pub fn modify(&self, game_id: GameID, action: GameModifyAction) {
        self.0.send(Message::ModifyGame(game_id, action)).ok();
    }
    pub fn remove_player(&self, game_id: GameID, ty: RemovePlayerType) {
        self.0.send(Message::RemovePlayer(game_id, ty)).ok();
    }
}

pub enum TryAddResult {
    Success,
    Failure(GamePlayer),
}

impl GameManager {
    /// Handling function for processing incoming messages from
    /// the matchmaking addr instances
    pub async fn process(mut self) {
        while let Some(message) = self.rx.recv().await {
            match message {
                Message::Create {
                    attributes,
                    setting,
                    host,
                    tx,
                } => {
                    let addr = self.create_game(attributes, setting, host);
                    tx.send(addr).ok();
                }
                Message::TryAdd(player, rule_set, tx) => {
                    let result = self.try_add(player, rule_set).await;
                    tx.send(result).ok();
                }
                Message::ModifyGame(game_id, action) => self.modify(game_id, action),
                Message::RemovePlayer(game_id, ty) => self.remove_player(game_id, ty).await,
                Message::SnapshotQuery { offset, count, tx } => {
                    self.snapshot_query(offset, count, tx)
                }
                Message::Snapshot { game_id, tx } => self.snapshot(game_id, tx),
            }
        }
    }

    pub fn snapshot(&self, game_id: GameID, tx: oneshot::Sender<Option<GameSnapshot>>) {
        let addr = match self.games.get(&game_id) {
            Some(value) => value.clone(),
            None => return,
        };

        tokio::spawn(async move {
            let snapshot = addr.snapshot().await;
            tx.send(snapshot).ok();
        });
    }

    pub fn snapshot_query(
        &self,
        offset: usize,
        count: usize,
        tx: oneshot::Sender<(Vec<GameSnapshot>, bool)>,
    ) {
        let mut join_set = JoinSet::new();
        let (count, more) = {
            // Obtained an order set of the keys from the games map
            let mut keys: Vec<GameID> = self.games.keys().copied().collect();
            keys.sort();

            // Whether there is more keys that what was requested
            let more = keys.len() > offset + count;

            // Collect the keys we will be using
            let keys: Vec<GameID> = keys.into_iter().skip(offset).take(count).collect();
            let keys_count = keys.len();

            for key in keys {
                let game = self.games.get(&key).cloned();
                if let Some(game) = game {
                    join_set.spawn(async move {
                        let game = game;
                        game.snapshot().await
                    });
                }
            }

            (keys_count, more)
        };

        tokio::spawn(async move {
            let mut snapshots = Vec::with_capacity(count);
            while let Some(result) = join_set.join_next().await {
                if let Ok(Some(snapshot)) = result {
                    snapshots.push(snapshot);
                }
            }

            tx.send((snapshots, more)).ok();
        });
    }

    pub fn create_game(&mut self, attributes: AttrMap, setting: u16, host: GamePlayer) -> GameAddr {
        let id = self.next_id;
        self.next_id += 1;
        let addr = GameAddr::spawn(id, attributes, setting);
        self.games.insert(id, addr.clone());
        addr.add_player(host);
        addr
    }

    pub async fn try_add(&self, player: GamePlayer, rule_set: Arc<RuleSet>) -> TryAddResult {
        for (id, addr) in &self.games {
            let join_state = addr.check_joinable(rule_set.clone()).await;
            if let GameJoinableState::Joinable = join_state {
                debug!("Found matching game (GID: {})", id);
                addr.add_player(player);
                return TryAddResult::Success;
            }
        }
        TryAddResult::Failure(player)
    }

    pub fn modify(&self, game_id: GameID, action: GameModifyAction) {
        if let Some(addr) = self.games.get(&game_id) {
            addr.send(action);
        }
    }

    pub async fn remove_player(&mut self, game_id: GameID, ty: RemovePlayerType) {
        if let Some(game) = self.games.get(&game_id) {
            let is_empty = game.remove_player(ty).await;
            if is_empty {
                // Remove the empty game
                self.games.remove(&game_id);
            }
        }
    }
}
