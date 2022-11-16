use blaze_pk::packet::Packet;
use core::blaze::components::Components;
use core::blaze::errors::HandleResult;
use log::debug;

use crate::session::Session;

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
        Components::Stats(value) => stats::route(session, value, packet).await,
        Components::Util(value) => util::route(session, value, packet).await,
        Components::Messaging(value) => messaging::route(session, value, packet).await,
        Components::UserSessions(value) => user_sessions::route(session, value, packet).await,
        Components::AssociationLists(value) => {
            other::route_association_lists(session, value, packet).await
        }
        Components::GameReporting(value) => {
            other::route_game_reporting(session, value, packet).await
        }
        value => {
            debug!("No handler for component {value:?}");
            session.response_empty(packet).await
        }
    }
}
