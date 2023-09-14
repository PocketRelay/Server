use crate::{
    database::entities::Player,
    services::sessions::{Sessions, VerifyError},
    session::{
        models::{
            auth::{AuthResponse, AuthenticationError},
            errors::ServerResult,
            user_sessions::*,
            NetworkAddress,
        },
        router::{Blaze, Extension},
        LookupResponse, SessionLink,
    },
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

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
pub async fn handle_lookup_user(
    Blaze(req): Blaze<LookupRequest>,
    Extension(sessions): Extension<Arc<Sessions>>,
) -> ServerResult<Blaze<LookupResponse>> {
    // Lookup the session

    let session = sessions
        .lookup_session(req.player_id)
        .await
        .ok_or(UserSessionsError::UserNotFound)?;

    // Get the lookup response from the session
    let response = session
        .get_lookup()
        .await
        .ok_or(UserSessionsError::UserNotFound)?;

    Ok(Blaze(response))
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
    session: SessionLink,
    Extension(db): Extension<DatabaseConnection>,
    Extension(sessions): Extension<Arc<Sessions>>,
    Blaze(req): Blaze<ResumeSessionRequest>,
) -> ServerResult<Blaze<AuthResponse>> {
    let session_token = req.session_token;

    // Verify the authentication token
    let player_id = sessions
        .verify_token(&session_token)
        .map_err(|err| match err {
            VerifyError::Expired => AuthenticationError::ExpiredToken,
            VerifyError::Invalid => AuthenticationError::InvalidToken,
        })?;

    let player = Player::by_id(&db, player_id)
        .await?
        .ok_or(AuthenticationError::InvalidToken)?;

    session.set_player(Some(player.clone())).await;
    sessions.add_session(player.id, session).await;

    Ok(Blaze(AuthResponse {
        player,
        session_token,
        silent: true,
    }))
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
pub async fn handle_update_network(
    session: SessionLink,
    Blaze(mut req): Blaze<UpdateNetworkRequest>,
) {
    if let NetworkAddress::AddressPair(pair) = &mut req.address {
        let ext = &mut pair.external;

        // If address is missing
        if ext.addr.is_unspecified() {
            // Replace address with new address and port with same as local port
            ext.addr = session.addr;
            ext.port = pair.internal.port;
        }
    }

    tokio::spawn(async move {
        session.set_network_info(req.address, req.qos).await;
    });
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
pub async fn handle_update_hardware_flag(
    session: SessionLink,
    Blaze(req): Blaze<UpdateHardwareFlagsRequest>,
) {
    tokio::spawn(async move {
        session.set_hardware_flags(req.hardware_flags).await;
    });
}
