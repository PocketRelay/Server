mod blaze;
mod env;
mod http;
mod utils;
mod database;
mod game;

use std::io;
use std::sync::Arc;
use dotenvy::dotenv;
use env_logger::WriteStyle;
use log::info;
use sea_orm::DatabaseConnection;
use tokio::try_join;
use crate::game::GameManager;

/// Global state that is shared throughout the application
pub struct GlobalState {
    pub game_manager: GameManager,
    pub db: DatabaseConnection,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    dotenv().ok();

    env_logger::builder()
        .filter_module("pocket_relay", env::logging_level())
        .write_style(WriteStyle::Always)
        .init();

    info!("Starting Pocket Relay v{}", env::VERSION);

    let db = database::connect().await?;
    let game_manager = GameManager::new();
    let global_state = GlobalState { db, game_manager };
    let global_state = Arc::new(global_state);

    try_join!(
        http::start_server(global_state.clone()),
        blaze::start_server(global_state)
    )?;

    Ok(())
}

