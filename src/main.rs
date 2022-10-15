mod blaze;
mod env;
mod http;

use std::io;
use dotenvy::dotenv;
use env_logger::WriteStyle;
use log::info;
use tokio::try_join;
use blaze::components::{Authentication, Components};

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

