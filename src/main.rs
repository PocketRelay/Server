mod blaze;
mod config;

use std::io;
use std::sync::Arc;
use blaze_pk::{CodecError, OpaquePacket, Packet, packet, PacketComponent, PacketComponents, PacketContent};
use env_logger::WriteStyle;
use log::{info, LevelFilter};
use blaze::components::{Authentication, Components};
use crate::blaze::start_server;


#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter_module("pocket_relay", LevelFilter::Info)
        .write_style(WriteStyle::Always)
        .init();


    info!("Message to logger");

   start_server().await.unwrap();
}

pub struct AppContext {
    name: String,
}



