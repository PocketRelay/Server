use crate::blaze::{components::Components, errors::BlazeError};
use blaze_pk::packet::Packet;

use super::session::SessionAddr;

mod auth;
mod game_manager;
mod messaging;
mod other;
mod stats;
mod user_sessions;
mod util;

/// Type alias for result from a routing function. Routing functions either
/// return a Packet tor esponse with or an Error
pub type HandleResult = Result<Packet, BlazeError>;

/// Root routing function handles the different components passing each
/// component onto its specific route function in its module.
///
/// `session`   The session to route the packet for
/// `component` The component of the packet
/// `packet`    The packet itself
pub async fn route(session: SessionAddr, component: Components, packet: &Packet) -> HandleResult {
    match component {
        Components::Authentication(value) => auth::route(session, value, packet).await,
        Components::GameManager(value) => game_manager::route(session, value, packet).await,
        Components::Stats(value) => stats::route(value, packet).await,
        Components::Util(value) => util::route(session, value, packet).await,
        Components::Messaging(value) => messaging::route(session, value, packet).await,
        Components::UserSessions(value) => user_sessions::route(session, value, packet).await,
        Components::AssociationLists(value) => other::route_assoc_lists(value, packet),
        Components::GameReporting(value) => other::route_game_reporting(session, value, packet),
        _ => Ok(packet.respond_empty()),
    }
}
