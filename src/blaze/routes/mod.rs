use crate::blaze::components::Components;
use crate::blaze::errors::{BlazeError, HandleResult};
use crate::blaze::SessionArc;
use blaze_pk::{OpaquePacket, Packets};
use log::{debug, error};

mod auth;
mod game_manager;
mod messaging;
mod other;
mod stats;
mod user_sessions;
mod util;

pub async fn route(
    session: &SessionArc,
    component: Components,
    packet: &OpaquePacket,
) -> HandleResult {
    let result = match component {
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
            packet.debug_decode()?;
            session
                .write_packet(&Packets::response_empty(packet))
                .await?;
            Ok(())
        }
    };

    if let Err(BlazeError::Response(response)) = &result {
        error!("Sending error response");
        // Send error responses
        session.write_packet(response).await?;
        return Ok(());
    }

    result
}
