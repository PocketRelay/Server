use super::{
    player::GamePlayer, rules::RuleSet, AttrMap, GameAddr, GameJoinableState, GameModifyAction,
    GameSnapshot, RemovePlayerType,
};
use crate::utils::types::GameID;
use interlink::{
    msg::{FutureResponse, MessageResponse, ServiceFutureResponse},
    prelude::*,
};
use log::debug;
use std::{collections::HashMap, sync::Arc};
use tokio::task::JoinSet;

/// Manager which controls all the active games on the server
/// commanding them to do different actions and removing them
/// once they are no longer used
struct GameManager {
    /// The map of games to the actual game address
    games: HashMap<GameID, GameAddr>,
    /// Stored value for the ID to give the next game
    next_id: u32,
}

impl Service for GameManager {}

pub struct GameManagerLink(Link<GameManager>);

impl GameManagerLink {
    pub fn start() -> GameManagerLink {
        let this = GameManager {
            games: Default::default(),
            next_id: 1,
        };
        let link = this.start();
        GameManagerLink(link)
    }

    pub async fn create(
        &self,
        attributes: AttrMap,
        setting: u16,
        host: GamePlayer,
    ) -> Option<GameAddr> {
        self.0
            .send(Create {
                attributes,
                setting,
                host,
            })
            .await
            .ok()
    }

    pub async fn try_add(
        &self,
        player: GamePlayer,
        rule_set: Arc<RuleSet>,
    ) -> Option<TryAddResult> {
        self.0.send(TryAdd { player, rule_set }).await.ok()
    }

    pub async fn snapshot(&self, game_id: GameID) -> Option<GameSnapshot> {
        self.0.send(Snapshot { game_id }).await.ok().flatten()
    }

    pub async fn snapshot_query(
        &self,
        offset: usize,
        count: usize,
    ) -> Option<(Vec<GameSnapshot>, bool)> {
        self.0.send(SnapshotQuery { offset, count }).await.ok()
    }

    pub fn modify(&self, game_id: GameID, action: GameModifyAction) {
        self.0.do_send(Modify { game_id, action }).ok();
    }
    pub fn remove_player(&self, game_id: GameID, ty: RemovePlayerType) {
        self.0.do_send(RemovePlayer { game_id, ty }).ok();
    }
}

struct SnapshotQuery {
    offset: usize,
    count: usize,
}

impl Message for SnapshotQuery {
    type Response = (Vec<GameSnapshot>, bool);
}

impl Handler<SnapshotQuery> for GameManager {
    type Response = FutureResponse<SnapshotQuery>;

    fn handle(&mut self, msg: SnapshotQuery, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        let mut join_set = JoinSet::new();
        let (count, more) = {
            // Obtained an order set of the keys from the games map
            let mut keys: Vec<GameID> = self.games.keys().copied().collect();
            keys.sort();

            // Whether there is more keys that what was requested
            let more = keys.len() > msg.offset + msg.count;

            // Collect the keys we will be using
            let keys: Vec<GameID> = keys.into_iter().skip(msg.offset).take(msg.count).collect();
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

        FutureResponse::new(Box::pin(async move {
            let mut snapshots = Vec::with_capacity(count);
            while let Some(result) = join_set.join_next().await {
                if let Ok(Some(snapshot)) = result {
                    snapshots.push(snapshot);
                }
            }
            (snapshots, more)
        }))
    }
}

struct Snapshot {
    game_id: GameID,
}

impl Message for Snapshot {
    type Response = Option<GameSnapshot>;
}

impl Handler<Snapshot> for GameManager {
    type Response = FutureResponse<Snapshot>;

    fn handle(&mut self, msg: Snapshot, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        let addr = self.games.get(&msg.game_id).cloned();

        FutureResponse::new(Box::pin(async move {
            let addr = match addr {
                Some(value) => value,
                None => return None,
            };
            addr.snapshot().await
        }))
    }
}

struct Create {
    attributes: AttrMap,
    setting: u16,
    host: GamePlayer,
}

impl Message for Create {
    type Response = GameAddr;
}

impl Handler<Create> for GameManager {
    type Response = MessageResponse<GameAddr>;

    fn handle(&mut self, msg: Create, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        let id = self.next_id;
        self.next_id += 1;
        let addr = GameAddr::spawn(id, msg.attributes, msg.setting);
        self.games.insert(id, addr.clone());
        addr.add_player(msg.host);
        MessageResponse(addr)
    }
}

struct Modify {
    game_id: GameID,
    action: GameModifyAction,
}

impl Message for Modify {
    type Response = ();
}

impl Handler<Modify> for GameManager {
    type Response = ();

    fn handle(&mut self, msg: Modify, _ctx: &mut ServiceContext<Self>) {
        if let Some(addr) = self.games.get(&msg.game_id) {
            addr.send(msg.action);
        }
    }
}

struct TryAdd {
    player: GamePlayer,
    rule_set: Arc<RuleSet>,
}

impl Message for TryAdd {
    type Response = TryAddResult;
}

impl Handler<TryAdd> for GameManager {
    type Response = ServiceFutureResponse<Self, TryAdd>;

    fn handle(&mut self, msg: TryAdd, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        ServiceFutureResponse::new(move |service: &mut GameManager, _ctx| {
            Box::pin(async move {
                for (id, addr) in &service.games {
                    let join_state = addr.check_joinable(msg.rule_set.clone()).await;
                    if let GameJoinableState::Joinable = join_state {
                        debug!("Found matching game (GID: {})", id);
                        addr.add_player(msg.player);
                        return TryAddResult::Success;
                    }
                }
                TryAddResult::Failure(msg.player)
            })
        })
    }
}

struct RemovePlayer {
    game_id: GameID,
    ty: RemovePlayerType,
}

impl Message for RemovePlayer {
    type Response = ();
}

impl Handler<RemovePlayer> for GameManager {
    type Response = ServiceFutureResponse<Self, RemovePlayer>;

    fn handle(&mut self, msg: RemovePlayer, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        ServiceFutureResponse::new(move |service: &mut GameManager, _ctx| {
            Box::pin(async move {
                if let Some(game) = service.games.get(&msg.game_id) {
                    let is_empty = game.remove_player(msg.ty).await;
                    if is_empty {
                        // Remove the empty game
                        service.games.remove(&msg.game_id);
                    }
                }
            })
        })
    }
}

pub enum TryAddResult {
    Success,
    Failure(GamePlayer),
}
