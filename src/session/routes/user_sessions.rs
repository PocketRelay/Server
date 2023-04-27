use crate::{
    database::entities::Player,
    services::{sessions::LookupMessage, tokens::Tokens},
    session::{
        models::{
            auth::AuthResponse,
            errors::{ServerError, ServerResult},
            user_sessions::*,
        },
        GetLookupMessage, HardwareFlagMessage, LookupResponse, NetworkInfoMessage, SessionLink,
        SetPlayerMessage,
    },
    state::App,
};
use log::error;

/// Attempts to lookup another authenticated session details
///
/// ```
/// Component: UserSessions(LookupUser)
/// Type: Request
/// ID: 70
/// Content: {
///     "AID": 0,
///     "ALOC": 0,
///     "EXBB": Blob [],
///     "EXID": 0,
///     "ID": 397394528,
///     "NAME": "",
/// }
/// ```
pub async fn handle_lookup_user(req: LookupRequest) -> ServerResult<LookupResponse> {
    let services = App::services();

    // Lookup the session
    let session = services
        .sessions
        .send(LookupMessage {
            player_id: req.player_id,
        })
        .await;

    // Ensure there wasn't an error
    let session = match session {
        Ok(Some(value)) => value,
        _ => return Err(ServerError::InvalidInformation),
    };

    // Get the lookup response from the session
    let response = session.send(GetLookupMessage {}).await;
    let response = match response {
        Ok(Some(value)) => value,
        _ => return Err(ServerError::InvalidInformation),
    };

    Ok(response)
}

/// Attempts to resume an existing session for a player that has the
/// provided session token.
///
/// ```
/// Route: UserSessions(ResumeSession)
/// ID: 207
/// Content: {
///     "SKEY": "127_CHARACTER_TOKEN"
/// }
/// ```
pub async fn handle_resume_session(
    session: &mut SessionLink,
    req: ResumeSessionRequest,
) -> ServerResult<AuthResponse> {
    let db = App::database();

    let session_token = req.session_token;

    let player: Player = match Tokens::service_verify(db, &session_token).await {
        Ok(value) => value,
        Err(err) => {
            error!("Error while attempt to resume session: {err:?}");
            return Err(ServerError::InvalidSession);
        }
    };

    // Failing to set the player likely the player disconnected or
    // the server is shutting down
    if session
        .send(SetPlayerMessage(Some(player.clone())))
        .await
        .is_err()
    {
        return Err(ServerError::ServerUnavailable);
    }

    Ok(AuthResponse {
        player,
        session_token,
        silent: true,
    })
}

/// Handles updating the stored networking information for the current session
/// this is required for clients to be able to connect to each-other
///
/// ```
/// Route: UserSessions(UpdateNetworkInfo)
/// ID: 8
/// Content: {
///     "ADDR": Union("VALUE", 2, {
///         "EXIP": {
///             "IP": 0,
///             "PORT": 0
///         },
///         "INIP": {
///             "IP": 0,
///             "PORT": 0
///         }
///     }),
///     "NLMP": Map { // Map of latency to Quality of service servers
///         "ea-sjc": 156,
///         "rs-iad": 0xFFF0FFF
///         "rs-lhr": 0xFFF0FFF
///     }
///     "NQOS": {
///         "DBPS": 0,
///         "NATT": 4,
///         "UBPS": 0
///     }
/// }
/// ```
pub async fn handle_update_network(session: &mut SessionLink, req: UpdateNetworkRequest) {
    let _ = session
        .send(NetworkInfoMessage {
            groups: req.address,
            qos: req.qos,
        })
        .await;
}

/// Handles updating the stored hardware flag with the client provided hardware flag
///
/// ```
/// Route: UserSessions(UpdateHardwareFlags)
/// ID: 22
/// Content: {
///     "HWFG": 0
/// }
/// ```
pub async fn handle_update_hardware_flag(session: &mut SessionLink, req: HardwareFlagRequest) {
    let _ = session
        .send(HardwareFlagMessage {
            value: req.hardware_flag,
        })
        .await;
}
