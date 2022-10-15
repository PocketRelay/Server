use blaze_pk::{CodecError, OpaquePacket, PacketContent, Packets};
use derive_more::From;
use log::debug;
use sea_orm::DbErr;
use tokio::io;
use crate::blaze::components::Components;
use crate::blaze::{Session, write_packet};

mod util;
mod auth;
mod game_manager;
mod stats;

#[derive(Debug, From)]
pub enum HandleError {
    CodecError(CodecError),
    IO(io::Error),
    Other(&'static str),
    Database(DbErr)
}

pub type HandleResult = Result<Option<OpaquePacket>, HandleError>;

pub async fn route(session: &Session, component: Components, packet: OpaquePacket) -> Result<(), HandleError> {
    let response = match component {
        Components::Authentication(value) => auth::route(&session, value, &packet).await,
        Components::GameManager(value) => game_manager::route(&session, value, &packet).await,
        Components::Stats(value) => stats::route(&session, value, &packet).await,
        Components::Util(value) => util::route(&session, value, &packet).await,
        value => {
            debug!("No handler for component {value:?}");
            packet.debug_decode()?;
            Ok(None)
        }
    }?;
    let response = response.unwrap_or_else(|| Packets::response_empty(&packet));
    write_packet(&session, response).await?;
    Ok(())
}

#[inline]
pub fn response<T: PacketContent>(packet: &OpaquePacket, contents: T) -> HandleResult {
    Ok(Some(Packets::response(packet, contents)))
}