mod blaze;
mod env;
mod http;

use std::io;
use std::sync::Arc;
use blaze_pk::{CodecError, OpaquePacket, Packet, packet, PacketComponent, PacketComponents, PacketContent};
use derive_more::From;
use dotenvy::dotenv;
use env_logger::WriteStyle;
use log::{info, LevelFilter};
use tokio::try_join;
use blaze::components::{Authentication, Components};

pub struct AppContext {
    name: String,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    dotenv().ok();
    let log_level = env::logging_level();
    env_logger::builder()
        .filter_module("pocket_relay", log_level)
        .write_style(WriteStyle::Always)
        .init();

    info!("Starting Pocket Relay v{}", env::VERSION);

    try_join!(
        http::start_server(),
        blaze::start_server()
    )?;

    Ok(())
}

