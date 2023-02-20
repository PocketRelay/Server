use super::{
    models::PlayerState, player::GamePlayer, rules::RuleSet, AddPlayerMessage, AttrMap,
    CheckJoinableMessage, Game, GameJoinableState, GameSnapshot, RemovePlayerType,
};
use crate::utils::types::GameID;
use futures::FutureExt;
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

impl Service for GameManager {}

/// Message for taking a snapshot of multiple games
/// within the specified query range
pub struct SnapshotQueryMessage {
    pub offset: usize,
    pub count: usize,
}

impl Message for SnapshotQueryMessage {
    /// Response of the list of games and whether
    /// there are more games at the next offset
    type Response = (Vec<GameSnapshot>, bool);
}

impl Handler<SnapshotQueryMessage> for GameManager {
    type Response = FutureResponse<SnapshotQueryMessage>;

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
                    join_set.spawn(async move { link.send(super::SnapshotMessage).await.ok() });
                }
            }

            (keys_count, more)
        };

        FutureResponse::new(
            async move {
                let mut snapshots = Vec::with_capacity(count);
                while let Some(result) = join_set.join_next().await {
                    if let Ok(Some(snapshot)) = result {
                        snapshots.push(snapshot);
                    }
                }
                (snapshots, more)
            }
            .boxed(),
        )
    }
}

/// Message for taking a snapshot of a specific game
pub struct SnapshotMessage {
    /// The ID of the game to take the snapshot of
    pub game_id: GameID,
}

impl Message for SnapshotMessage {
    /// Response of an optional game snapshot if the game exists
    type Response = Option<GameSnapshot>;
}

impl Handler<SnapshotMessage> for GameManager {
    type Response = FutureResponse<SnapshotMessage>;

    fn handle(&mut self, msg: SnapshotMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        // Link to the game
        let link = self.games.get(&msg.game_id).cloned();

        FutureResponse::new(
            async move {
                let link = match link {
                    Some(value) => value,
                    None => return None,
                };

                link.send(super::SnapshotMessage).await.ok()
            }
            .boxed(),
        )
    }
}

/// Message for creating a new game using the game manager
pub struct CreateMessage {
    /// The initial game attributes
    pub attributes: AttrMap,
    /// The initial game setting
    pub setting: u16,
    /// The host player for the game
    pub host: GamePlayer,
}

impl Message for CreateMessage {
    /// Create message responds with the address of the
    /// created game
    type Response = (Link<Game>, GameID);
}

impl Handler<CreateMessage> for GameManager {
    type Response = MessageResponse<CreateMessage>;

    fn handle(
        &mut self,
        mut msg: CreateMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        let id = self.next_id;
        self.next_id += 1;
        let link = Game::start(id, msg.attributes, msg.setting);
        self.games.insert(id, link.clone());

        msg.host.state = PlayerState::Connected;

        let _ = link.do_send(AddPlayerMessage { player: msg.host });

        MessageResponse((link, id))
    }
}

/// Message for requesting a link to a game
/// with the provided ID
pub struct GetGameMessage {
    /// The ID of the game to get a link to
    pub game_id: GameID,
}

impl Message for GetGameMessage {
    /// Response is an option of a link to a game
    type Response = Option<Link<Game>>;
}

impl Handler<GetGameMessage> for GameManager {
    type Response = MessageResponse<GetGameMessage>;

    fn handle(&mut self, msg: GetGameMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        let link = self.games.get(&msg.game_id).cloned();
        MessageResponse(link)
    }
}

/// Message for attempting to add a player to any existing
/// games within this game manager
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
    Success,
    Failure(GamePlayer),
}

impl Message for TryAddMessage {
    /// Respond with a TryAddResult
    type Response = TryAddResult;
}

impl Handler<TryAddMessage> for GameManager {
    type Response = ServiceFutureResponse<Self, TryAddMessage>;

    fn handle(&mut self, msg: TryAddMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        ServiceFutureResponse::new(move |service: &mut GameManager, _ctx| {
            async move {
                for (id, link) in &service.games {
                    let join_state = match link
                        .send(CheckJoinableMessage {
                            rule_set: msg.rule_set.clone(),
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
            }
            .boxed()
        })
    }
}

/// Message for removing a player from a game
pub struct RemovePlayerMessage {
    /// The ID of the game to remove from
    pub game_id: GameID,
    /// The type of player removal
    pub ty: RemovePlayerType,
}

impl Message for RemovePlayerMessage {
    /// Empty response type
    type Response = ();
}

impl Handler<RemovePlayerMessage> for GameManager {
    type Response = FutureResponse<RemovePlayerMessage>;

    fn handle(
        &mut self,
        msg: RemovePlayerMessage,
        ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        // Link back to the game manager
        let return_link = ctx.link();

        // Link to the target game
        let link = self.games.get(&msg.game_id).cloned();

        FutureResponse::new(
            async move {
                let link = match link {
                    Some(value) => value,
                    None => return,
                };

                let is_empty = match link.send(super::RemovePlayerMessage { ty: msg.ty }).await {
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
            }
            .boxed(),
        )
    }
}

/// Message for removing a game from the manager
pub struct RemoveGameMessage {
    /// The ID of the game to remove
    pub game_id: GameID,
}

impl Message for RemoveGameMessage {
    type Response = ();
}

impl Handler<RemoveGameMessage> for GameManager {
    type Response = MessageResponse<RemoveGameMessage>;

    fn handle(
        &mut self,
        msg: RemoveGameMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        // Remove the game
        if let Some(value) = self.games.remove(&msg.game_id) {
            value.stop();
        }
        MessageResponse(())
    }
}
