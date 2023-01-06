use std::net::{IpAddr, SocketAddr};

use crate::{
    blaze::{
        codec::NetAddress,
        components::{Components as C, UserSessions as U},
        errors::{ServerError, ServerResult},
    },
    servers::main::{
        models::{auth::AuthResponse, user_sessions::*},
        session::Session,
    },
    state::GlobalState,
    utils::net::public_address,
};
use blaze_pk::{
    packet::{Request, Response},
    router::Router,
};
use database::Player;
use log::error;

/// Routing function for adding all the routes in this file to the
/// provided router
///
/// `router` The router to add to
pub fn route(router: &mut Router<C, Session>) {
    router.route(C::UserSessions(U::ResumeSession), handle_resume_session);
    router.route(C::UserSessions(U::UpdateNetworkInfo), handle_update_network);
    router.route(
        C::UserSessions(U::UpdateHardwareFlags),
        handle_update_hardware_flag,
    );
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
async fn handle_resume_session(
    session: &mut Session,
    req: Request<ResumeSessionRequest>,
) -> ServerResult<Response> {
    let db = GlobalState::database();

    // Find the player that the token is for
    let player: Player = match Player::by_token(db, &req.session_token).await {
        // Valid session token
        Ok(Some(player)) => player,
        // Session that was attempted to resume is expired
        Ok(None) => return Err(ServerError::InvalidSession),
        // Error occurred while looking up token
        Err(err) => {
            error!("Error while attempt to resume session: {err:?}");
            return Err(ServerError::ServerUnavailable);
        }
    };

    let (player, session_token) = session.set_player(player).await?;

    let res = AuthResponse {
        player,
        session_token,
        silent: true,
    };

    Ok(req.response(res))
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
async fn handle_update_network(session: &mut Session, req: UpdateNetworkRequest) {
    let mut groups = req.address;
    let external = &mut groups.external;
    if external.0.is_invalid() || external.1 == 0 {
        // Match port with internal address
        external.1 = groups.internal.1;
        external.0 = get_network_address(&session.socket_addr).await;
    }

    session.set_network_info(groups, req.qos);
}

/// Obtains the networking address from the provided SocketAddr
/// if the address is a loopback or private address then the
/// public IP address of the network is used instead.
///
/// `value` The socket address
async fn get_network_address(addr: &SocketAddr) -> NetAddress {
    let ip = addr.ip();
    if let IpAddr::V4(value) = ip {
        // Address is already a public address
        if value.is_loopback() || value.is_private() {
            if let Some(public_addr) = public_address().await {
                return NetAddress::from_ipv4(&public_addr);
            }
        }
        let value = format!("{}", value);
        NetAddress::from_ipv4(&value)
    } else {
        NetAddress(0)
    }
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
async fn handle_update_hardware_flag(session: &mut Session, req: HardwareFlagRequest) {
    session.set_hardware_flag(req.hardware_flag);
}
