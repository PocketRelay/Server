use std::sync::Arc;

use crate::{
    services::{
        game::{
            manager::{
                CreateMessage, GetGameDataMessage, GetGameMessage, RemovePlayerMessage,
                TryAddMessage, TryAddResult,
            },
            player::GamePlayer,
            AddPlayerMessage, CheckJoinableMessage, GameJoinableState, RemovePlayerType,
            SetAttributesMessage, SetSettingMessage, SetStateMessage, UpdateMeshMessage,
        },
        matchmaking::{GameCreatedMessage, QueuePlayerMessage},
        sessions::LookupMessage,
    },
    session::{
        models::{
            errors::{ServerError, ServerResult},
            game_manager::*,
        },
        GetGamePlayerMessage, GetIdMessage, GetPlayerGameMessage, SessionLink,
    },
    state::GlobalState,
    utils::components::{Components as C, GameManager as G},
};
use blaze_pk::{packet::PacketBody, router::Router};
use log::{debug, info};

/// Routing function for adding all the routes in this file to the
/// provided router
///
/// `router` The router to add to
pub fn route(router: &mut Router<C, SessionLink>) {
    router.route(C::GameManager(G::CreateGame), handle_create_game);
    router.route(C::GameManager(G::AdvanceGameState), handle_set_state);
    router.route(C::GameManager(G::SetGameSettings), handle_set_setting);
    router.route(C::GameManager(G::SetGameAttributes), handle_set_attributes);
    router.route(C::GameManager(G::RemovePlayer), handle_remove_player);
    router.route(C::GameManager(G::RemovePlayer), handle_remove_player);
    router.route(
        C::GameManager(G::UpdateMeshConnection),
        handle_update_mesh_connection,
    );
    router.route(
        C::GameManager(G::StartMatchmaking),
        handle_start_matchmaking,
    );
    router.route(
        C::GameManager(G::CancelMatchmaking),
        handle_cancel_matchmaking,
    );
    router.route(C::GameManager(G::GetGameDataFromID), handle_get_game_data);
    router.route(C::GameManager(G::JoinGame), handle_join_game)
}

async fn handle_join_game(
    session: &mut SessionLink,
    req: JoinGameRequest,
) -> ServerResult<JoinGameResponse> {
    let services = GlobalState::services();

    // Load the session
    let player: GamePlayer = session
        .send(GetGamePlayerMessage)
        .await
        .map_err(|_| ServerError::ServerUnavailable)?
        .ok_or(ServerError::FailedNoLoginAction)?;

    // Lookup the session join target
    let session = services
        .sessions
        .send(LookupMessage {
            player_id: req.target_id,
        })
        .await;

    // Ensure there wasn't an error
    let session = match session {
        Ok(Some(value)) => value,
        _ => return Err(ServerError::InvalidInformation),
    };

    // Find the game ID for the target session
    let game_id = session.send(GetPlayerGameMessage {}).await;
    let game_id = match game_id {
        Ok(Some(value)) => value,
        _ => return Err(ServerError::InvalidInformation),
    };

    let game = services
        .game_manager
        .send(GetGameMessage { game_id })
        .await
        .map_err(|_| ServerError::ServerUnavailableFinal)?;

    let game = match game {
        Some(value) => value,
        None => return Err(ServerError::InvalidInformation),
    };

    // Check the game is joinable
    let join_state = match game.send(CheckJoinableMessage { rule_set: None }).await {
        Ok(value) => value,
        // Game is no longer available
        Err(_) => return Err(ServerError::InvalidInformation),
    };

    // Join the game
    if let GameJoinableState::Joinable = join_state {
        debug!("Joining game from invite (GID: {})", game_id);
        let _ = game.do_send(AddPlayerMessage { player });
        Ok(JoinGameResponse { game_id })
    } else {
        Err(ServerError::InvalidInformation)
    }
}

async fn handle_get_game_data(mut req: GetGameDataRequest) -> ServerResult<PacketBody> {
    let services = GlobalState::services();

    if req.game_list.is_empty() {
        return Err(ServerError::InvalidInformation);
    }

    let game_id = req.game_list.remove(0);

    let link = services
        .game_manager
        .send(GetGameDataMessage { game_id })
        .await
        .map_err(|_| ServerError::ServerUnavailableFinal)?;

    let link = match link {
        Some(value) => value,
        None => return Err(ServerError::InvalidInformation),
    };

    Ok(link)
}

/// Handles creating a game for the provided session.
///
/// ```
/// Route: GameManager(CreateGame)
/// ID: 55
/// Content: {
///     "ATTR": Map {
///         "ME3_dlc2300": "required"
///         "ME3_dlc2500": "required",
///         "ME3_dlc2700": "required",
///         "ME3_dlc3050": "required",
///         "ME3_dlc3225": "required",
///         "ME3gameDifficulty": "difficulty0",
///         "ME3gameEnemyType": "enemy1",
///         "ME3map": "map11",
///         "ME3privacy": "PUBLIC",
///     },
///     "BTPL": (0, 0, 0),
///     "GCTR": "",
///     "GENT": 0,
///     "GNAM": "test@test.com",
///     "GSET": 287,
///     "GTYP": "",
///     "GURL": "",
///     "HNET": Union(Group, 2, {
///         "EXIP": {
///             "IP": 0, // Encoded IP address
///             "PORT": 0 // Port
///         },
///         "INIP": {
///             "IP": 0, // Encoded IP address
///             "PORT": 0 // Port
///         }
///     } (2))
///     "IGNO": 0,
///     "NRES": 0,
///     "NTOP": 0,
///     "PCAP": [4, 0],
///     "PGID": "",
///     "PGSC": Blob [],
///     "PMAX": 4,
///     "PRES": 1,
///     "QCAP": 0,
///     "RGID": 0,
///     "SLOT": 0,
///     "TCAP": 0,
///     "TIDX": 0xFFFF,
///     "VOIP": 2,
///     "VSTR": "ME3-295976325-179181965240128"
/// }
/// ```
async fn handle_create_game(
    session: &mut SessionLink,
    req: CreateGameRequest,
) -> ServerResult<CreateGameResponse> {
    let player: GamePlayer = session
        .send(GetGamePlayerMessage)
        .await
        .map_err(|_| ServerError::ServerUnavailable)?
        .ok_or(ServerError::FailedNoLoginAction)?;
    let services = GlobalState::services();

    let (link, game_id) = match services
        .game_manager
        .send(CreateMessage {
            attributes: req.attributes,
            setting: req.setting,
            host: player,
        })
        .await
    {
        Ok(value) => value,
        Err(_) => return Err(ServerError::ServerUnavailable),
    };

    // Notify matchmaking of the new game
    let _ = services
        .matchmaking
        .do_send(GameCreatedMessage { link, game_id });

    Ok(CreateGameResponse { game_id })
}

/// Handles changing the attributes of the game with the provided ID
///
/// ```
/// Route: GameManager(SetGameAttributes)
/// ID: 162
/// Content: {
///     "ATTR": Map<String, String> {
///         "ME3_dlc2300": "required",
///         "ME3_dlc2500": "required",
///         "ME3_dlc2700": "required",
///         "ME3_dlc3050": "required",
///         "ME3_dlc3225": "required",
///         "ME3gameDifficulty": "difficulty0",
///         "ME3gameEnemyType": "enemy1",
///         "ME3map": "map2",
///         "ME3privacy": "PUBLIC",
///     },
///     "GID": 1
/// }
/// ```
async fn handle_set_attributes(req: SetAttributesRequest) -> ServerResult<()> {
    let services = GlobalState::services();
    let link = services
        .game_manager
        .send(GetGameMessage {
            game_id: req.game_id,
        })
        .await
        .map_err(|_| ServerError::ServerUnavailableFinal)?;

    if let Some(link) = link {
        link.send(SetAttributesMessage {
            attributes: req.attributes,
        })
        .await
        .map_err(|_| ServerError::InvalidInformation)?;
    }

    Ok(())
}

/// Handles changing the state of the game with the provided ID
///
/// ```
/// Route: GameManager(AdvanceGameState)
/// ID: 57
/// Content: {
///     "GID": 1
///     "GSTA": 130
/// }
/// ```
async fn handle_set_state(req: SetStateRequest) -> ServerResult<()> {
    let services = GlobalState::services();
    let link = services
        .game_manager
        .send(GetGameMessage {
            game_id: req.game_id,
        })
        .await
        .map_err(|_| ServerError::ServerUnavailableFinal)?;

    if let Some(link) = link {
        link.send(SetStateMessage { state: req.state })
            .await
            .map_err(|_| ServerError::InvalidInformation)?;
    }

    Ok(())
}

/// Handles changing the setting of the game with the provided ID
///
/// ```
/// Route: GameManager(SetGameSettings)
/// ID: 161
/// Content: {
///     "GID": 1,
///     "GSET": 285
/// }
/// ```
async fn handle_set_setting(req: SetSettingRequest) -> ServerResult<()> {
    let services = GlobalState::services();
    let link = services
        .game_manager
        .send(GetGameMessage {
            game_id: req.game_id,
        })
        .await
        .map_err(|_| ServerError::ServerUnavailableFinal)?;

    if let Some(link) = link {
        link.send(SetSettingMessage {
            setting: req.setting,
        })
        .await
        .map_err(|_| ServerError::InvalidInformation)?;
    }

    Ok(())
}

/// Handles removing a player from a game
///
/// ```
/// Route: GameManager(RemovePlayer)
/// ID: 151
/// Content: {
///     "BTPL": (0, 0, 0),
///     "CNTX": 0,
///     "GID": 1,
///     "PID": 1,
///     "REAS": 6
/// }
/// ```
async fn handle_remove_player(req: RemovePlayerRequest) {
    let services = GlobalState::services();
    let _ = services
        .game_manager
        .send(RemovePlayerMessage {
            game_id: req.game_id,
            reason: req.reason,
            id: req.player_id,
            ty: RemovePlayerType::Player,
        })
        .await;
}

/// Handles updating mesh connections
///
/// ```
/// Route: GameManager(UpdateMeshConnection)
/// ID: 147
/// Content: {
///     "GID": 1,
///     "TARG": [
///         {
///             "FLGS": 0,
///             "PID": 1,
///             "STAT": 2
///         }
///     ]
/// }
/// ```
async fn handle_update_mesh_connection(
    session: &mut SessionLink,
    req: UpdateMeshRequest,
) -> ServerResult<()> {
    let id = match session.send(GetIdMessage).await {
        Ok(value) => value,
        Err(_) => return Err(ServerError::ServerUnavailable),
    };

    let target = match req.target {
        Some(value) => value,
        None => return Ok(()),
    };

    let services = GlobalState::services();

    let link = services
        .game_manager
        .send(GetGameMessage {
            game_id: req.game_id,
        })
        .await
        .map_err(|_| ServerError::ServerUnavailableFinal)?;

    let link = match link {
        Some(value) => value,
        None => return Ok(()),
    };

    let _ = link
        .send(UpdateMeshMessage {
            session: id,
            target: target.player_id,
            state: target.state,
        })
        .await;

    Ok(())
}

/// Handles either directly joining a game or placing the
/// session into a matchmaking queue for searching for games.
///
/// ```
/// Route: GameManager(StartMatchmaking)
/// ID: 146
/// Content: {
///     "BTPL": (0, 0, 0),
///     "CRIT": {
///         "CUST": {},
///         "DNF": { "DNF": 101 },
///         "GEO": { "THLD": "" },
///         "GNAM": { "SUBS": "" },
///         "NAT": { "THLD": "hostBalancing" },
///         "PSR": { "THLD": "" },
///         "RANK": { "THLD": "" },
///         "RLST": [
///             {
///                 "NAME": "ME3_gameStateMatchRule",
///                 "THLD": "quickMatch",   
///                 "VALU": ["MATCH_MAKING"]
///             },
///             {
///                 "NAME": "ME3_gameMapMatchRule",
///                 "THLD": "quickMatch",   
///                 "VALU": ["abstain"]
///             },
///             {
///                 "NAME": "ME3_gameEnemyTypeRule",
///                 "THLD": "quickMatch",   
///                 "VALU": ["abstain"]
///             },
///             {
///                 "NAME": "ME3_gameDifficultyRule",
///                 "THLD": "quickMatch",   
///                 "VALU": ["abstain"]
///             },
///             {
///                 "NAME": "ME3_rule_dlc2500",
///                 "THLD": "requireExactMatch",   
///                 "VALU": ["required"]
///             },
///             {
///                 "NAME": "ME3_rule_dlc2300",
///                 "THLD": "requireExactMatch",   
///                 "VALU": ["required"]
///             },
///             {
///                 "NAME": "ME3_rule_dlc2700",
///                 "THLD": "requireExactMatch",   
///                 "VALU": ["required"]
///             },
///             {
///                 "NAME": "ME3_rule_dlc3050",
///                 "THLD": "requireExactMatch",   
///                 "VALU": ["required"]
///             },
///             {
///                 "NAME": "ME3_rule_dlc3225",
///                 "THLD": "requireExactMatch",   
///                 "VALU": ["required"]
///             },
///         ],
///         "RSZR": {
///             "PCAP": 65535,
///             "PMIN": 0
///         },
///         "SIZE": {
///             "ISSG": 0,
///             "PCAP": 4,
///             "PCNT": 4,
///             "PMIN": 2,
///             "THLD": "matchAny"
///         },
///         "TEAM": {
///             "PCAP": 0,
///             "PCNT": 0,
///             "PMIN": 0,
///             "SDIF": 0,
///             "THLD": "",
///             "TID": 65535      
///         },
///         "UED": Map {
///             "ME3_characterSkill_Rule": {
///                 "CVAL": 0,
///                 "NAME": "ME3_characterSkill_Rule",
///                 "OVAL": 0,
///                 "THLD": "quickMatch"
///             }
///         },
///         "VIAB": {
///             "THLD": "hostViability"    
///         },
///         "VIRT": {
///             "THLD": "",
///             "VALUE": 1
///         }
///     },
///     "DUR": 1800000,
///     "GENT": 0,
///     "GNAM": "",
///     "GSET": 1311,
///     "GVER": "ME3-295976325-179181965240128",
///     "IGNO": 0,
///     "MODE": 3
///     "NTOP": 0,
///     "PMAX": 0,
///     "PNET": Union("VALU", 2, {
///         "EXIP": {
///             "IP": 0,
///             "PORT": 0,
///         },
///         "INIP": {
///             "IP": 0,
///             "PORT": 0
///         }
///     }),
///     "QCAP": 0,
///     "VOIP": 2
/// }
/// ```
async fn handle_start_matchmaking(
    session: &mut SessionLink,
    req: MatchmakingRequest,
) -> ServerResult<MatchmakingResponse> {
    let player: GamePlayer = session
        .send(GetGamePlayerMessage)
        .await
        .map_err(|_| ServerError::ServerUnavailable)?
        .ok_or(ServerError::FailedNoLoginAction)?;

    let session_id = player.session_id;

    info!("Player {} started matchmaking", player.player.display_name);

    let services = GlobalState::services();

    let rule_set = Arc::new(req.rules);

    let result = match services
        .game_manager
        .send(TryAddMessage {
            player,
            rule_set: rule_set.clone(),
        })
        .await
    {
        Ok(value) => value,
        Err(_) => return Err(ServerError::ServerUnavailable),
    };

    // If adding failed attempt to queue instead
    if let TryAddResult::Failure(player) = result {
        if services
            .matchmaking
            .send(QueuePlayerMessage { player, rule_set })
            .await
            .is_err()
        {
            return Err(ServerError::ServerUnavailable);
        }
    }

    Ok(MatchmakingResponse { id: session_id })
}

/// Handles cancelling matchmaking for the current session removing
/// itself from the matchmaking queue.
///
/// ```
/// Route: GameManager(CancelMatchmaking)
/// ID: 84
/// Content: {
///     "MSID": 1
/// }
/// ```
async fn handle_cancel_matchmaking(session: &mut SessionLink) {
    session
        .exec(|session, _| {
            let services = GlobalState::services();
            session.remove_games(services);
        })
        .await
        .ok();
}
