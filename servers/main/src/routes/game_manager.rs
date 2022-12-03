use crate::models::game_manager::{
    CreateGameRequest, CreateGameResponse, GameModifyRequest, MatchmakingRequest,
    MatchmakingResponse, RemovePlayerRequest, UpdateMeshRequest,
};
use crate::routes::HandleResult;
use crate::session::Session;
use blaze_pk::packet::Packet;
use core::blaze::components::GameManager;
use core::blaze::errors::ServerError;
use core::game::player::GamePlayer;
use core::state::GlobalState;
use database::Player;
use log::{debug, info, warn};
use utils::types::GameID;

/// Routing function for handling packets with the `GameManager` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
///
/// `session`   The session that the packet was recieved by
/// `component` The component of the packet recieved
/// `packet`    The recieved packet
pub async fn route(session: &mut Session, component: GameManager, packet: &Packet) -> HandleResult {
    match component {
        GameManager::CreateGame => handle_create_game(session, packet).await,
        GameManager::AdvanceGameState
        | GameManager::SetGameSettings
        | GameManager::SetGameAttributes => handle_game_modify(session, packet).await,
        GameManager::RemovePlayer => handle_remove_player(session, packet).await,
        GameManager::UpdateMeshConnection => handle_update_mesh_connection(session, packet).await,
        GameManager::StartMatchaking => handle_start_matchmaking(session, packet).await,
        GameManager::CancelMatchmaking => handle_cancel_matchmaking(session, packet).await,
        _ => Ok(packet.respond_empty()),
    }
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
async fn handle_create_game(session: &mut Session, packet: &Packet) -> HandleResult {
    let req: CreateGameRequest = packet.decode()?;

    let player: GamePlayer = session
        .try_into_player()
        .ok_or(ServerError::FailedNoLoginAction)?;

    let games = GlobalState::games();
    let game_id: GameID = games.create_game(req.attributes, req.setting).await;

    games.add_host(game_id, player).await;

    let response = CreateGameResponse { game_id };
    Ok(packet.respond(response))
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
///
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
async fn handle_game_modify(session: &mut Session, packet: &Packet) -> HandleResult {
    let req: GameModifyRequest = packet.decode()?;
    let games = GlobalState::games();
    let (game_id, result) = match req {
        GameModifyRequest::State(game_id, state) => {
            (game_id, games.set_game_state(game_id, state).await)
        }
        GameModifyRequest::Setting(game_id, setting) => {
            (game_id, games.set_game_setting(game_id, setting).await)
        }
        GameModifyRequest::Attributes(game_id, attributes) => (
            game_id,
            games.set_game_attributes(game_id, attributes).await,
        ),
    };

    if !result {
        warn!(
            "Client requested to modify the state of an unknown game (GID: {}, SID: {})",
            game_id, session.id
        );
        return Err(ServerError::InvalidInformation.into());
    }

    Ok(packet.respond_empty())
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
async fn handle_remove_player(session: &mut Session, packet: &Packet) -> HandleResult {
    let req: RemovePlayerRequest = packet.decode()?;
    let games = GlobalState::games();
    if !games
        .remove_player_pid(req.game_id, req.player_id, req.reason)
        .await
    {
        warn!(
            "Client requested to advance the game state of an unknown game (GID: {}, SID: {})",
            req.game_id, session.id
        );
        return Err(ServerError::InvalidInformation.into());
    }

    Ok(packet.respond_empty())
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
async fn handle_update_mesh_connection(session: &mut Session, packet: &Packet) -> HandleResult {
    let req: UpdateMeshRequest = packet.decode()?;
    let target = match req.targets.first() {
        Some(value) => *value,
        None => return Ok(packet.respond_empty()),
    };

    let games = GlobalState::games();
    if !games
        .update_mesh_connection(req.game_id, session.id, target)
        .await
    {
        warn!(
            "Client requested to advance the game state of an unknown game (GID: {}, SID: {})",
            req.game_id, session.id
        );
    }

    Ok(packet.respond_empty())
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
async fn handle_start_matchmaking(session: &mut Session, packet: &Packet) -> HandleResult {
    let req: MatchmakingRequest = packet.decode()?;

    let player: GamePlayer = session
        .try_into_player()
        .ok_or(ServerError::FailedNoLoginAction)?;

    info!("Player {} started matchmaking", player.display_name);

    let games = GlobalState::games();
    if games.add_or_queue(player, req.rules).await {
        debug!("Matchmaking Ended")
    }

    let response = MatchmakingResponse { id: session.id };
    Ok(packet.respond(response))
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
async fn handle_cancel_matchmaking(session: &mut Session, packet: &Packet) -> HandleResult {
    let player: &Player = session
        .player
        .as_ref()
        .ok_or(ServerError::FailedNoLoginAction)?;
    info!("Player {} cancelled matchmaking", player.display_name);

    let games = GlobalState::games();
    if let Some(game) = session.game.as_ref() {
        games.remove_player_sid(*game, session.id).await;
    } else {
        games.unqueue_session(session.id).await;
    }

    Ok(packet.respond_empty())
}
