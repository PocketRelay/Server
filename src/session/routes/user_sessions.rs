use interlink::prelude::Link;
use sea_orm::DatabaseConnection;

use crate::{
    database::entities::Player,
    services::{
        sessions::{AuthedSessions, LookupMessage},
        tokens::Tokens,
    },
    session::{
        models::{
            auth::AuthResponse,
            errors::{GlobalError, ServerResult},
            user_sessions::*,
        },
        router::{Blaze, Extension},
        GetLookupMessage, GetSocketAddrMessage, HardwareFlagMessage, LookupResponse,
        NetworkInfoMessage, SessionLink, SetPlayerMessage,
    },
    utils::models::NetworkAddress,
};
use std::{net::SocketAddr, sync::Arc};

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
    Extension(sessions): Extension<Link<AuthedSessions>>,
) -> ServerResult<Blaze<LookupResponse>> {
    // Lookup the session
    let session = sessions
        .send(LookupMessage {
            player_id: req.player_id,
        })
        .await;

    // Ensure there wasn't an error
    let session = match session {
        Ok(Some(value)) => value,
        _ => return Err(GlobalError::System.into()),
    };

    // Get the lookup response from the session
    let response = session.send(GetLookupMessage {}).await;
    let response = match response {
        Ok(Some(value)) => value,
        _ => return Err(GlobalError::System.into()),
    };

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
    Extension(tokens): Extension<Arc<Tokens>>,
    Blaze(req): Blaze<ResumeSessionRequest>,
) -> ServerResult<Blaze<AuthResponse>> {
    let session_token = req.session_token;

    let player: Player = tokens.verify_player(&db, &session_token).await?;

    // Failing to set the player likely the player disconnected or
    // the server is shutting down
    session.send(SetPlayerMessage(Some(player.clone()))).await?;

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
            // Obtain socket address from session
            if let Ok(SocketAddr::V4(addr)) = session.send(GetSocketAddrMessage).await {
                let ip = addr.ip();
                // Replace address with new address and port with same as local port
                ext.addr = *ip;
                ext.port = pair.internal.port;
            }
        }
    }

    let _ = session
        .send(NetworkInfoMessage {
            address: req.address,
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
pub async fn handle_update_hardware_flag(
    session: SessionLink,
    Blaze(req): Blaze<HardwareFlagRequest>,
) {
    let _ = session
        .send(HardwareFlagMessage {
            value: req.hardware_flag,
        })
        .await;
}
