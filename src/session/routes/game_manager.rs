use crate::{
    services::{
        game::{manager::GameManager, GameJoinableState, GamePlayer},
        sessions::Sessions,
    },
    session::{
        models::{
            errors::{GlobalError, ServerResult},
            game_manager::*,
        },
        router::{Blaze, Extension, RawBlaze, SessionAuth},
        SessionLink,
    },
};
use log::{debug, info};
use std::sync::Arc;

pub async fn handle_join_game(
    player: GamePlayer,
    Blaze(JoinGameRequest { user }): Blaze<JoinGameRequest>,
    Extension(sessions): Extension<Arc<Sessions>>,
    Extension(game_manager): Extension<Arc<GameManager>>,
) -> ServerResult<Blaze<JoinGameResponse>> {
    // Lookup the session join target
    let session = sessions
        .lookup_session(user.id)
        .await
        .ok_or(GameManagerError::JoinPlayerFailed)?;

    // Find the game ID for the target session
    let (game_id, game_ref) = session
        .get_game()
        .await
        .ok_or(GameManagerError::InvalidGameId)?;

    // Check the game is joinable
    let join_state = {
        let game = &*game_ref.read().await;
        game.joinable_state(None)
    };

    // Join the game
    if let GameJoinableState::Joinable = join_state {
        debug!("Joining game from invite (GID: {})", game_id);

        game_manager
            .add_to_game(
                game_ref,
                player,
                GameSetupContext::Dataless {
                    context: DatalessContext::JoinGameSetup,
                },
            )
            .await;

        Ok(Blaze(JoinGameResponse {
            game_id,
            state: JoinGameState::JoinedGame,
        }))
    } else {
        Err(GameManagerError::GameFull.into())
    }
}

pub async fn handle_get_game_data(
    Blaze(GetGameDataRequest { game_list }): Blaze<GetGameDataRequest>,
    Extension(game_manager): Extension<Arc<GameManager>>,
) -> ServerResult<RawBlaze> {
    let game_id = game_list.first().copied().ok_or(GlobalError::System)?;
    let game = game_manager
        .get_game(game_id)
        .await
        .ok_or(GameManagerError::InvalidGameId)?;

    let game = &*game.read().await;
    let body = game.game_data().await;

    Ok(body)
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
pub async fn handle_create_game(
    player: GamePlayer,
    Extension(game_manager): Extension<Arc<GameManager>>,
    Blaze(CreateGameRequest {
        attributes,
        setting,
    }): Blaze<CreateGameRequest>,
) -> ServerResult<Blaze<CreateGameResponse>> {
    let (link, game_id) = game_manager.create_game(attributes, setting).await;

    // Notify matchmaking of the new game
    tokio::spawn(async move {
        game_manager
            .add_to_game(
                link.clone(),
                player,
                GameSetupContext::Dataless {
                    context: DatalessContext::CreateGameSetup,
                },
            )
            .await;

        // Update matchmaking with the new game
        game_manager.process_queue(link, game_id).await;
    });

    Ok(Blaze(CreateGameResponse { game_id }))
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
pub async fn handle_set_attributes(
    Extension(game_manager): Extension<Arc<GameManager>>,
    Blaze(SetAttributesRequest {
        attributes,
        game_id,
    }): Blaze<SetAttributesRequest>,
) -> ServerResult<()> {
    let link = game_manager
        .get_game(game_id)
        .await
        .ok_or(GameManagerError::InvalidGameId)?;

    {
        let game = &mut *link.write().await;
        game.set_attributes(attributes);
    }

    // Update matchmaking for the changed game
    tokio::spawn(async move {
        let join_state = {
            let game = &*link.read().await;
            game.joinable_state(None)
        };
        if let GameJoinableState::Joinable = join_state {
            game_manager.process_queue(link, game_id).await;
        }
    });

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
pub async fn handle_set_state(
    Extension(game_manager): Extension<Arc<GameManager>>,
    Blaze(SetStateRequest { game_id, state }): Blaze<SetStateRequest>,
) -> ServerResult<()> {
    let link = game_manager
        .get_game(game_id)
        .await
        .ok_or(GameManagerError::InvalidGameId)?;

    tokio::spawn(async move {
        let game = &mut *link.write().await;
        game.set_state(state);
    });

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
pub async fn handle_set_setting(
    Extension(game_manager): Extension<Arc<GameManager>>,
    Blaze(SetSettingRequest { game_id, setting }): Blaze<SetSettingRequest>,
) -> ServerResult<()> {
    let link = game_manager
        .get_game(game_id)
        .await
        .ok_or(GameManagerError::InvalidGameId)?;

    tokio::spawn(async move {
        let game = &mut *link.write().await;
        game.set_settings(setting);
    });

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
pub async fn handle_remove_player(
    Extension(game_manager): Extension<Arc<GameManager>>,
    Blaze(RemovePlayerRequest {
        game_id,
        player_id,
        reason,
    }): Blaze<RemovePlayerRequest>,
) -> ServerResult<()> {
    let link = game_manager
        .get_game(game_id)
        .await
        .ok_or(GameManagerError::InvalidGameId)?;

    tokio::spawn(async move {
        let game = &mut *link.write().await;
        game.remove_player(player_id, reason).await;
    });

    Ok(())
}

/// Handles updating mesh connections
///
/// Only sent by the host player (I think)
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
pub async fn handle_update_mesh_connection(
    SessionAuth(player): SessionAuth,
    Extension(game_manager): Extension<Arc<GameManager>>,
    Blaze(UpdateMeshRequest {
        game_id,
        mut targets,
    }): Blaze<UpdateMeshRequest>,
) -> ServerResult<()> {
    let target = match targets.pop() {
        Some(value) => value,
        None => return Ok(()),
    };

    let link = game_manager
        .get_game(game_id)
        .await
        .ok_or(GameManagerError::InvalidGameId)?;

    tokio::spawn(async move {
        let game = &mut *link.write().await;

        // Ensure the host is the one making the change
        if !game.is_host_player(player.id) {
            return;
        }

        game.update_mesh(target.player_id, target.status);
    });

    Ok(())
}

pub async fn handle_add_admin_player(
    SessionAuth(player): SessionAuth,
    Extension(game_manager): Extension<Arc<GameManager>>,
    Blaze(AddAdminPlayerRequest { game_id, player_id }): Blaze<AddAdminPlayerRequest>,
) -> ServerResult<()> {
    let link = game_manager
        .get_game(game_id)
        .await
        .ok_or(GameManagerError::InvalidGameId)?;

    tokio::spawn(async move {
        let game = &mut *link.write().await;

        // Ensure the host is the one making the change
        if !game.is_host_player(player.id) {
            return;
        }

        game.add_admin_player(player_id);
    });

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
pub async fn handle_start_matchmaking(
    player: GamePlayer,
    Extension(game_manager): Extension<Arc<GameManager>>,
    Blaze(MatchmakingRequest { rules }): Blaze<MatchmakingRequest>,
) -> ServerResult<Blaze<MatchmakingResponse>> {
    let session_id = player.player.id;

    info!("Player {} started matchmaking", player.player.display_name);

    tokio::spawn(async move {
        let rule_set = Arc::new(rules);
        // If adding failed attempt to queue instead
        if let Err(player) = game_manager.try_add(player, &rule_set).await {
            game_manager.queue(player, rule_set).await;
        }
    });

    Ok(Blaze(MatchmakingResponse { id: session_id }))
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
pub async fn handle_cancel_matchmaking(
    session: SessionLink,
    SessionAuth(player): SessionAuth,
    Extension(game_manager): Extension<Arc<GameManager>>,
) {
    session.remove_from_game().await;
    game_manager.remove_queue(player.id).await;
}
