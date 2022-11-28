use crate::{
    models::{
        auth::AuthResponse,
        user_sessions::{HardwareFlagRequest, ResumeSessionRequest, UpdateNetworkRequest},
    },
    session::Session,
    HandleResult,
};
use blaze_pk::packet::Packet;
use core::blaze::errors::ServerError;
use core::{blaze::components::UserSessions, state::GlobalState};
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
        UserSessions::UpdateHardwareFlags => handle_update_hardware_flag(session, packet).await,
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
    let player = Player::by_token(db, &req.session_token)
        .await?
        .ok_or(ServerError::InvalidSession)?;

    let (player, session_token) = player.with_token(db).await?;
    let player = session.set_player(player);
    let response = AuthResponse::new(player, session_token, true);
    Ok(packet.respond(&response))
}

/// Handles updating the stored networking information for the current session
/// this is required for clients to be able to connect to each-other
///
/// ```
/// Route: UserSessions(UpdateNetworkInfo)
/// ID: 8
/// Content: {
///     "ADDR": Union("VALUE", 2: {
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
    session.set_network_info(req.address, req.qos).await;
    Ok(packet.respond_empty())
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
async fn handle_update_hardware_flag(session: &mut Session, packet: &Packet) -> HandleResult {
    let req: HardwareFlagRequest = packet.decode()?;
    session.set_hardware_flag(req.hardware_flag);
    Ok(packet.respond_empty())
}
