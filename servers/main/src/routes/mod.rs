use blaze_pk::packet::Packet;
use core::blaze::components::Components;

use crate::{session::Session, HandleResult};

mod auth;
mod game_manager;
mod messaging;
mod other;
mod stats;
mod user_sessions;
mod util;

pub async fn route(session: &mut Session, component: Components, packet: &Packet) -> HandleResult {
    match component {
        Components::Authentication(value) => auth::route(session, value, packet).await,
        Components::GameManager(value) => game_manager::route(session, value, packet).await,
        Components::Stats(value) => stats::route(session, value, packet),
        Components::Util(value) => util::route(session, value, packet).await,
        Components::Messaging(value) => messaging::route(session, value, packet).await,
        Components::UserSessions(value) => user_sessions::route(session, value, packet).await,
        Components::AssociationLists(value) => other::route_assoc_lists(value, packet),
        Components::GameReporting(value) => other::route_game_reporting(session, value, packet),
        _ => Ok(packet.respond_empty()),
    }
}
