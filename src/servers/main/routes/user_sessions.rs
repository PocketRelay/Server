use std::net::{IpAddr, SocketAddr};

use crate::{
    blaze::{codec::NetAddress, components::UserSessions, errors::ServerError},
    servers::main::{
        models::{auth::AuthResponse, user_sessions::*},
        routes::HandleResult,
        session::Session,
    },
    state::GlobalState,
    utils::net::public_address,
};
use blaze_pk::packet::Packet;
use database::Player;

/// Routing function for handling packets with the `Stats` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
///
/// `session`   The session that the packet was recieved by
/// `component` The component of the packet recieved
/// `packet`    The recieved packet
pub async fn route(
    session: &mut Session,
    component: UserSessions,
    packet: &Packet,
) -> HandleResult {
    match component {
        UserSessions::ResumeSession => handle_resume_session(session, packet).await,
        UserSessions::UpdateNetworkInfo => handle_update_network_info(session, packet).await,
        UserSessions::UpdateHardwareFlags => handle_update_hardware_flag(session, packet),
        _ => Ok(packet.respond_empty()),
    }
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
async fn handle_resume_session(session: &mut Session, packet: &Packet) -> HandleResult {
    let req: ResumeSessionRequest = packet.decode()?;
    let db = GlobalState::database();
    let player: Player = Player::by_token(db, &req.session_token)
        .await?
        .ok_or(ServerError::InvalidSession)?;

    let (player, session_token) = session.set_player(db, player).await?;
    let response = AuthResponse::new(player, session_token, true);
    Ok(packet.respond(response))
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
async fn handle_update_network_info(session: &mut Session, packet: &Packet) -> HandleResult {
    let req: UpdateNetworkRequest = packet.decode()?;

    // TODO: Possibly spawn this off into a task and have a session
    // message update the networking information when this is complete?

    let mut groups = req.address;
    let external = &mut groups.external;
    if external.0.is_invalid() || external.1 == 0 {
        // Match port with internal address
        external.1 = groups.internal.1;
        external.0 = get_network_address(&session.addr).await;
    }

    session.set_network_info(groups, req.qos);
    Ok(packet.respond_empty())
}

/// Obtains the networking address from the provided SocketAddr
/// if the address is a loopback or private address then the
/// public IP address of the network is used instead.
///
/// `value` The socket address
async fn get_network_address(addr: &SocketAddr) -> NetAddress {
    let ip = addr.ip();
    if let IpAddr::V4(value) = ip {
        // Value is local or private
        if value.is_loopback() || value.is_private() {
            if let Some(public_addr) = public_address().await {
                return NetAddress::from_ipv4(&public_addr);
            }
        }
        let value = format!("{}", value);
        NetAddress::from_ipv4(&value)
    } else {
        // Don't know how to handle IPv6 addresses
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
fn handle_update_hardware_flag(session: &mut Session, packet: &Packet) -> HandleResult {
    let req: HardwareFlagRequest = packet.decode()?;
    session.set_hardware_flag(req.hardware_flag);
    Ok(packet.respond_empty())
}
