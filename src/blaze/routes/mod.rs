use std::sync::Arc;
use blaze_pk::{CodecError, OpaquePacket};
use derive_more::From;
use log::debug;
use crate::blaze::components::Components;
use super::Session;

mod util;
mod auth;
mod game_manager;
mod stats;

#[derive(Debug, From)]
pub enum HandleError {
    CodecError(CodecError)
}

pub type HandleResult = Result<(), HandleError>;

pub async fn route(session: Arc<Session>, component: Components, packet: OpaquePacket) -> HandleResult {
    packet.debug_decode()?;
    match component {
        Components::Authentication(value) => auth::route(session, value, packet).await,
        Components::GameManager(value) => game_manager::route(session, value, packet).await,
        Components::Stats(value) =>  stats::route(session, value, packet).await,
        Components::Util(value) => util::route(session, value, packet).await,
        value => {
            debug!("No handler for component {value:?}");
            return Ok(())
        }
    }
}