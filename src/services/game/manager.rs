use super::{
    models::{MeshState, RemoveReason},
    player::GamePlayer,
    AddPlayerMessage, AttrMap, CheckJoinableMessage, Game, GameJoinableState, GameSnapshot,
    RemovePlayerType,
};
use crate::{services::matchmaking::rules::RuleSet, utils::types::GameID};
use blaze_pk::packet::PacketBody;
use interlink::prelude::*;
use log::debug;
use std::{collections::HashMap, sync::Arc};
use tokio::task::JoinSet;

/// Manager which controls all the active games on the server
/// commanding them to do different actions and removing them
/// once they are no longer used
#[derive(Service)]
pub struct GameManager {
    /// The map of games to the actual game address
    games: HashMap<GameID, Link<Game>>,
    /// Stored value for the ID to give the next game
    next_id: u32,
}

impl GameManager {
    /// Starts a new game manager service returning its link
    pub fn start() -> Link<GameManager> {
        let this = GameManager {
            games: Default::default(),
            next_id: 1,
        };
        this.start()
    }
}

/// Message for taking a snapshot of multiple games
/// within the specified query range
#[derive(Message)]
#[msg(rtype = "(Vec<GameSnapshot>, bool)")]
pub struct SnapshotQueryMessage {
    /// The offset to start querying games from
    pub offset: usize,
    /// The number of games to query
    pub count: usize,
    /// Whether to include sensitively player net info
    pub include_net: bool,
}

/// Handler for snapshot query messages
impl Handler<SnapshotQueryMessage> for GameManager {
    type Response = Fr<SnapshotQueryMessage>;

    fn handle(
        &mut self,
        msg: SnapshotQueryMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        // Create the futures using the handle action before passing
        // them to a future to be awaited
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
                if let Some(link) = game {
                    join_set.spawn(async move {
                        link.send(super::SnapshotMessage {
                            include_net: msg.include_net,
                        })
                        .await
                        .ok()
                    });
                }
            }

            (keys_count, more)
        };

        Fr::new(Box::pin(async move {
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

/// Message for taking a snapshot of a specific game
/// which will return a snapshot of the game if it
/// exists
#[derive(Message)]
#[msg(rtype = "Option<GameSnapshot>")]
pub struct SnapshotMessage {
    /// The ID of the game to take the snapshot of
    pub game_id: GameID,
    /// Whether to include sensitively player net info
    pub include_net: bool,
}

/// Handler for snapshot messages for a specific game
impl Handler<SnapshotMessage> for GameManager {
    type Response = Fr<SnapshotMessage>;

    fn handle(&mut self, msg: SnapshotMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        // Link to the game
        let link = self.games.get(&msg.game_id).cloned();

        Fr::new(Box::pin(async move {
            let link = match link {
                Some(value) => value,
                None => return None,
            };

            link.send(super::SnapshotMessage {
                include_net: msg.include_net,
            })
            .await
            .ok()
        }))
    }
}

/// Message for creating a new game using the game manager
/// responds with a link to the created game and its ID
#[derive(Message)]
#[msg(rtype = "(Link<Game>, GameID)")]
pub struct CreateMessage {
    /// The initial game attributes
    pub attributes: AttrMap,
    /// The initial game setting
    pub setting: u16,
    /// The host player for the game
    pub host: GamePlayer,
}

/// Handler for creating games
impl Handler<CreateMessage> for GameManager {
    type Response = Mr<CreateMessage>;

    fn handle(
        &mut self,
        mut msg: CreateMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        let id = self.next_id;
        self.next_id += 1;
        msg.host.state = MeshState::Connected;

        let link = Game::start(id, msg.attributes, msg.setting);
        self.games.insert(id, link.clone());

        let _ = link.do_send(AddPlayerMessage { player: msg.host });

        Mr((link, id))
    }
}

/// Message for requesting a link to a game with the provided
/// ID responds with a link to the game if it exists
#[derive(Message)]
#[msg(rtype = "Option<Link<Game>>")]
pub struct GetGameMessage {
    /// The ID of the game to get a link to
    pub game_id: GameID,
}

/// Handler for getting a specific game
impl Handler<GetGameMessage> for GameManager {
    type Response = Mr<GetGameMessage>;

    fn handle(&mut self, msg: GetGameMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        let link = self.games.get(&msg.game_id).cloned();
        Mr(link)
    }
}

/// Message for attempting to add a player to any existing
/// games within this game manager
#[derive(Message)]
#[msg(rtype = "TryAddResult")]
pub struct TryAddMessage {
    /// The player to attempt to add
    pub player: GamePlayer,
    /// The set of rules the player requires the game has
    pub rule_set: Arc<RuleSet>,
}

/// Result of attempting to add a player. Success will
/// consume the game player and Failure will return the
/// game player back
pub enum TryAddResult {
    /// The player was added to the game
    Success,
    /// The player failed to be added and was returned back
    Failure(GamePlayer),
}

/// Handler for attempting to add a player
impl Handler<TryAddMessage> for GameManager {
    type Response = Sfr<Self, TryAddMessage>;

    fn handle(&mut self, msg: TryAddMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        Sfr::new(move |service: &mut GameManager, _ctx| {
            Box::pin(async move {
                for (id, link) in &service.games {
                    let join_state = match link
                        .send(CheckJoinableMessage {
                            rule_set: Some(msg.rule_set.clone()),
                        })
                        .await
                    {
                        Ok(value) => value,
                        // Game is no longer available
                        Err(_) => continue,
                    };

                    if let GameJoinableState::Joinable = join_state {
                        debug!("Found matching game (GID: {})", id);
                        let _ = link.do_send(AddPlayerMessage { player: msg.player });

                        return TryAddResult::Success;
                    }
                }
                TryAddResult::Failure(msg.player)
            })
        })
    }
}

/// Message for removing a player from a game
#[derive(Message)]
pub struct RemovePlayerMessage {
    /// The ID of the game to remove from
    pub game_id: GameID,
    /// The ID of the player (Session or PID depending on RemovePlayerType)
    pub id: u32,
    /// The reason for removing the player
    pub reason: RemoveReason,
    /// The type of player removal
    pub ty: RemovePlayerType,
}

/// Handler for removing a player from a game
impl Handler<RemovePlayerMessage> for GameManager {
    type Response = Fr<RemovePlayerMessage>;

    fn handle(
        &mut self,
        msg: RemovePlayerMessage,
        ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        // Link back to the game manager
        let return_link = ctx.link();

        // Link to the target game
        let link = self.games.get(&msg.game_id).cloned();

        Fr::new(Box::pin(async move {
            let link = match link {
                Some(value) => value,
                None => return,
            };

            let is_empty = match link
                .send(super::RemovePlayerMessage {
                    id: msg.id,
                    reason: msg.reason,
                    ty: msg.ty,
                })
                .await
            {
                Ok(value) => value,
                Err(_) => return,
            };

            if is_empty {
                // Remove the empty game
                let _ = return_link
                    .send(RemoveGameMessage {
                        game_id: msg.game_id,
                    })
                    .await;
            }
        }))
    }
}

/// Message for removing a game from the manager
#[derive(Message)]
pub struct RemoveGameMessage {
    /// The ID of the game to remove
    pub game_id: GameID,
}

/// Handler for removing a game
impl Handler<RemoveGameMessage> for GameManager {
    type Response = ();

    fn handle(&mut self, msg: RemoveGameMessage, _ctx: &mut ServiceContext<Self>) {
        // Remove the game
        if let Some(value) = self.games.remove(&msg.game_id) {
            value.stop();
        }
    }
}

/// Message for getting the encoded packet data for a game. Used
/// by the game lookup messages from Origin invites
#[derive(Message)]
#[msg(rtype = "Option<PacketBody>")]
pub struct GetGameDataMessage {
    /// The ID of the game to get the data for
    pub game_id: GameID,
}

/// Handler for getting game data
impl Handler<GetGameDataMessage> for GameManager {
    type Response = Fr<GetGameDataMessage>;

    fn handle(
        &mut self,
        msg: GetGameDataMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        let link = self.games.get(&msg.game_id).cloned();

        let link = match link {
            Some(value) => value,
            None => return Fr::ready(None),
        };

        Fr::new(Box::pin(async move {
            let data = link.send(super::GetGameDataMessage {}).await.ok()?;
            Some(data)
        }))
    }
}
