use crate::{
    config::Config,
    services::{
        game::{
            matchmaking::Matchmaking, store::Games, Game, GameAddPlayerExt, GameJoinableState,
            GamePlayer,
        },
        sessions::Sessions,
        tunnel::TunnelService,
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
    session: SessionLink,
    Blaze(JoinGameRequest { user }): Blaze<JoinGameRequest>,
    Extension(sessions): Extension<Arc<Sessions>>,
    Extension(tunnel_service): Extension<Arc<TunnelService>>,
    Extension(config): Extension<Arc<Config>>,
) -> ServerResult<Blaze<JoinGameResponse>> {
    // Lookup the session join target
    let other_session = sessions
        .lookup_session(user.id)
        .ok_or(GameManagerError::JoinPlayerFailed)?;

    // Find the game ID for the target session
    let (game_id, game_ref) = other_session
        .data
        .get_game()
        .ok_or(GameManagerError::InvalidGameId)?;

    // Check the game is joinable
    let join_state = { game_ref.read().joinable_state(None) };

    if !matches!(join_state, GameJoinableState::Joinable) {
        return Err(GameManagerError::GameFull.into());
    }

    // Join the game
    debug!("Joining game from invite (GID: {})", game_id);

    game_ref.add_player(
        &tunnel_service,
        &config,
        player,
        session,
        GameSetupContext::Dataless {
            context: DatalessContext::JoinGameSetup,
        },
    );

    Ok(Blaze(JoinGameResponse {
        game_id,
        state: JoinGameState::JoinedGame,
    }))
}

pub async fn handle_get_game_data(
    Blaze(GetGameDataRequest { game_list }): Blaze<GetGameDataRequest>,
    Extension(games): Extension<Arc<Games>>,
) -> ServerResult<RawBlaze> {
    let game_id = game_list.first().copied().ok_or(GlobalError::System)?;
    let game = games
        .get_by_id(game_id)
        .ok_or(GameManagerError::InvalidGameId)?;

    let body = game.read().game_data();

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
    session: SessionLink,
    Extension(games): Extension<Arc<Games>>,
    Extension(matchmaking): Extension<Arc<Matchmaking>>,
    Extension(tunnel_service): Extension<Arc<TunnelService>>,
    Extension(config): Extension<Arc<Config>>,
    Blaze(CreateGameRequest {
        attributes,
        setting,
    }): Blaze<CreateGameRequest>,
) -> ServerResult<Blaze<CreateGameResponse>> {
    let game_id = games.next_id();
    let game = Game::new(
        game_id,
        attributes,
        setting,
        games.clone(),
        tunnel_service.clone(),
    );
    let game_ref = games.insert(game);

    // Notify matchmaking of the new game
    let mut player = player;

    // Player is the host player (They are connected by default)
    player.state = PlayerState::ActiveConnected;

    // Add player to the game
    game_ref.add_player(
        &tunnel_service,
        &config,
        player,
        session,
        GameSetupContext::Dataless {
            context: DatalessContext::CreateGameSetup,
        },
    );

    // Update matchmaking with the new game

    matchmaking.process_queue(&tunnel_service, &config, &game_ref, game_id);

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
    Extension(games): Extension<Arc<Games>>,
    Extension(matchmaking): Extension<Arc<Matchmaking>>,
    Extension(tunnel_service): Extension<Arc<TunnelService>>,
    Extension(config): Extension<Arc<Config>>,

    Blaze(SetAttributesRequest {
        attributes,
        game_id,
    }): Blaze<SetAttributesRequest>,
) -> ServerResult<()> {
    let game_ref = games
        .get_by_id(game_id)
        .ok_or(GameManagerError::InvalidGameId)?;

    // Update matchmaking for the changed game
    let join_state = {
        let game = &mut *game_ref.write();
        game.set_attributes(attributes);
        game.joinable_state(None)
    };

    if let GameJoinableState::Joinable = join_state {
        matchmaking.process_queue(&tunnel_service, &config, &game_ref, game_id);
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
pub async fn handle_set_state(
    Extension(games): Extension<Arc<Games>>,
    Blaze(SetStateRequest { game_id, state }): Blaze<SetStateRequest>,
) -> ServerResult<()> {
    let game_ref = games
        .get_by_id(game_id)
        .ok_or(GameManagerError::InvalidGameId)?;

    game_ref.write().set_state(state);

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
    Extension(games): Extension<Arc<Games>>,
    Blaze(SetSettingRequest { game_id, setting }): Blaze<SetSettingRequest>,
) -> ServerResult<()> {
    let game_ref = games
        .get_by_id(game_id)
        .ok_or(GameManagerError::InvalidGameId)?;

    game_ref.write().set_settings(setting);

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
    Extension(games): Extension<Arc<Games>>,
    Blaze(RemovePlayerRequest {
        game_id,
        player_id,
        reason,
    }): Blaze<RemovePlayerRequest>,
) -> ServerResult<()> {
    let game_ref = games
        .get_by_id(game_id)
        .ok_or(GameManagerError::InvalidGameId)?;

    game_ref.write().remove_player(player_id, reason);

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
    Extension(games): Extension<Arc<Games>>,
    Blaze(UpdateMeshRequest {
        game_id,
        mut targets,
    }): Blaze<UpdateMeshRequest>,
) -> ServerResult<()> {
    let Some(target) = targets.pop() else {
        return Ok(());
    };

    let game_ref = games
        .get_by_id(game_id)
        .ok_or(GameManagerError::InvalidGameId)?;

    let game = &mut *game_ref.write();

    // Ensure the host is the one making the change
    if game.is_host_player(player.id) {
        game.update_mesh(target.player_id, target.status);
    }

    Ok(())
}

pub async fn handle_add_admin_player(
    SessionAuth(player): SessionAuth,
    Extension(games): Extension<Arc<Games>>,
    Blaze(AddAdminPlayerRequest { game_id, player_id }): Blaze<AddAdminPlayerRequest>,
) -> ServerResult<()> {
    let link = games
        .get_by_id(game_id)
        .ok_or(GameManagerError::InvalidGameId)?;

    let game = &mut *link.write();

    // Ensure the host is the one making the change
    if game.is_host_player(player.id) {
        game.add_admin_player(player_id);
    }

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
    Extension(games): Extension<Arc<Games>>,
    Extension(matchmaking): Extension<Arc<Matchmaking>>,
    Extension(tunnel_service): Extension<Arc<TunnelService>>,
    Extension(config): Extension<Arc<Config>>,

    Blaze(MatchmakingRequest { rules }): Blaze<MatchmakingRequest>,
) -> ServerResult<Blaze<MatchmakingResponse>> {
    let player_id = player.player.id;

    info!("Player {} started matchmaking", player.player.display_name);

    // Find a game thats currently joinable and matches the required rules
    match games.get_by_rule_set(&rules) {
        Some((game_id, game_ref)) => {
            debug!("Found matching game (GID: {})", game_id);

            // Add the player to the game
            matchmaking.add_from_matchmaking(&tunnel_service, &config, game_ref, player);
        }
        None => {
            matchmaking.queue(player, rules);
        }
    };

    Ok(Blaze(MatchmakingResponse { id: player_id }))
}

/// Handles cancelling matchmaking for the current session removing
/// itself from the matchmaking queue.
///
/// ```
/// Route: GameManager(CancelMatchmaking)
/// Content: {
///     "MSID": 1
/// }
/// ```
pub async fn handle_cancel_matchmaking(
    session: SessionLink,
    SessionAuth(player): SessionAuth,
    Extension(matchmaking): Extension<Arc<Matchmaking>>,
) {
    // Clear the current game
    session.data.clear_game();

    matchmaking.remove(player.id);
}

/// Handles preparing a game for being replayed
///
/// Occurs when a game is finished successfully, is not sent if a game
/// is failed
///
/// ```
/// Route: GameManager(ReplayGame)
/// Content: {
///  "GID": 2,
/// }
/// ```
#[allow(unused)]
pub async fn handle_replay_game(
    session: SessionLink,
    SessionAuth(player): SessionAuth,
    Extension(games): Extension<Arc<Games>>,

    Blaze(ReplayGameRequest { game_id }): Blaze<ReplayGameRequest>,
) -> ServerResult<()> {
    let game_ref = games
        .get_by_id(game_id)
        .ok_or(GameManagerError::InvalidGameId)?;
    Ok(())
}
