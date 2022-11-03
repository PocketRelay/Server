mod blaze;
mod database;
mod env;
mod game;
mod http;
mod retriever;
mod utils;

use crate::game::Games;
use dotenvy::dotenv;
use env_logger::WriteStyle;
use game::matchmaking::Matchmaking;
use log::info;
use retriever::Retriever;
use sea_orm::DatabaseConnection;
use std::io;
use std::sync::Arc;
use tokio::try_join;

/// Global state that is shared throughout the application
pub struct GlobalState {
    pub games: Games,
    pub matchmaking: Matchmaking,
    pub db: DatabaseConnection,
    pub retriever: Option<Retriever>,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    dotenv().ok();

    let logging_level = env::logging_level();

    env_logger::builder()
        .filter_module("pocket_relay", logging_level)
        .filter_module("actix_web", logging_level)
        .filter_module("actix", logging_level)
        .write_style(WriteStyle::Always)
        .init();

    info!("Starting Pocket Relay v{}", env::VERSION);

    let db = database::connect().await?;
    let games = Games::new();
    let matchmaking = Matchmaking::new();

    let retriever = Retriever::new().await;

    let global_state = GlobalState {
        db,
        games,
        matchmaking,
        retriever,
    };
    let global_state = Arc::new(global_state);

    try_join!(
        http::start_server(global_state.clone()),
        blaze::start_server(global_state)
    )?;

    Ok(())
}
