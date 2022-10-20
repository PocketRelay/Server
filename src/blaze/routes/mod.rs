use blaze_pk::{OpaquePacket, PacketContent, Packets};
use log::debug;
use crate::blaze::components::Components;
use crate::blaze::errors::{BlazeError, HandleResult};
use crate::blaze::Session;

mod util;
mod auth;
mod game_manager;
mod stats;

pub async fn route(session: &Session, component: Components, packet: &OpaquePacket) -> Result<(), BlazeError> {
    let result = match component {
        Components::Authentication(value) => auth::route(session, value, packet).await,
        Components::GameManager(value) => game_manager::route(session, value, packet).await,
        Components::Stats(value) => stats::route(session, value, packet).await,
        Components::Util(value) => util::route(session, value, packet).await,
        value => {
            debug!("No handler for component {value:?}");
            packet.debug_decode()?;
            session.write_packet(&Packets::response_empty(packet)).await?;
            Ok(())
        }
    };

    if let Err(BlazeError::Response(response)) = &result {
        // Send error responses
        session.write_packet(response).await?;
        return Ok(())
    }

    result
}

