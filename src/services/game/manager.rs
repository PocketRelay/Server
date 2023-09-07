use super::{
    models::{DatalessContext, GameSettings, GameSetupContext, PlayerState},
    AddPlayerMessage, AttrMap, CheckJoinableMessage, Game, GameJoinableState, GamePlayer,
    GameSnapshot,
};
use crate::{
    services::matchmaking::{rules::RuleSet, Matchmaking},
    utils::types::GameID,
};
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
    next_id: GameID,
    matchmaking: Link<Matchmaking>,
}

impl GameManager {
    /// Starts a new game manager service returning its link
    pub fn start(matchmaking: Link<Matchmaking>) -> Link<GameManager> {
        let this = GameManager {
            games: Default::default(),
            next_id: 1,
            matchmaking,
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
        let SnapshotQueryMessage {
            offset,
            count,
            include_net,
        } = msg;

        // Create the futures using the handle action before passing
        // them to a future to be awaited
        let mut join_set = JoinSet::new();

        // Obtained an order set of the keys from the games map
        let mut keys: Vec<&GameID> = self.games.keys().collect();
        keys.sort();

        // Whether there is more keys that what was requested
        let more = keys.len() > offset + count;

        // Spawn tasks for obtaining snapshots to each game
        keys.into_iter()
            // Skip to the desired offset
            .skip(offset)
            // Take the desired number of keys
            .take(count)
            // Take the game links for the keys
            .filter_map(|key| self.games.get(key))
            // Clone the obtained game links
            .cloned()
            // Spawn the snapshot tasks
            .for_each(|game| {
                join_set
                    .spawn(async move { game.send(super::SnapshotMessage { include_net }).await });
            });

        Fr::new(Box::pin(async move {
            // Allocate a list for the snapshots
            let mut snapshots = Vec::with_capacity(join_set.len());

            // Recieve all the snapshots from their tasks
            while let Some(result) = join_set.join_next().await {
                if let Ok(Ok(snapshot)) = result {
                    snapshots.push(snapshot);
                }
            }

            (snapshots, more)
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
    pub setting: GameSettings,
    /// The host player for the game
    pub host: GamePlayer,
}

/// Handler for creating games
impl Handler<CreateMessage> for GameManager {
    type Response = Mr<CreateMessage>;

    fn handle(&mut self, mut msg: CreateMessage, ctx: &mut ServiceContext<Self>) -> Self::Response {
        let id = self.next_id;

        self.next_id = self.next_id.wrapping_add(1);

        msg.host.state = PlayerState::ActiveConnected;

        let link = Game::start(
            id,
            msg.attributes,
            msg.setting,
            ctx.link(),
            self.matchmaking.clone(),
        );
        self.games.insert(id, link.clone());

        let _ = link.do_send(AddPlayerMessage {
            player: msg.host,
            context: GameSetupContext::Dataless(DatalessContext::CreateGameSetup),
        });

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
    type Response = Fr<TryAddMessage>;

    fn handle(&mut self, msg: TryAddMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        // Take a copy of the current games list
        let games = self.games.clone();

        Fr::new(Box::pin(async move {
            let player = msg.player;

            // Message asking for the game joinable state
            let msg = CheckJoinableMessage {
                rule_set: Some(msg.rule_set),
            };

            // Attempt to find a game thats joinable
            for (id, link) in games {
                // Check if the game is joinable
                if let Ok(GameJoinableState::Joinable) = link.send(msg.clone()).await {
                    debug!("Found matching game (GID: {})", id);
                    let msid = player.player.id;
                    let _ = link.do_send(AddPlayerMessage {
                        player,
                        context: GameSetupContext::Matchmaking(msid),
                    });
                    return TryAddResult::Success;
                }
            }

            TryAddResult::Failure(player)
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
