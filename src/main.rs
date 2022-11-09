mod blaze;
mod database;
mod env;
mod game;
mod http;
mod retriever;
mod utils;

use crate::game::Games;
use dotenvy::dotenv;
use game::matchmaking::Matchmaking;
use log::info;
use retriever::Retriever;
use sea_orm::DatabaseConnection;
use std::io;
use std::sync::Arc;
use tokio::sync::broadcast::{self, Receiver};
use tokio::{select, signal};

/// Global state that is shared throughout the application
pub struct GlobalState {
    pub games: Games,
    pub matchmaking: Matchmaking,
    pub db: DatabaseConnection,
    pub retriever: Option<Retriever>,
    pub shutdown: Receiver<()>,
}

pub type GlobalStateArc = Arc<GlobalState>;

#[tokio::main]
async fn main() -> io::Result<()> {
    dotenv().ok();

    utils::init_logger();

    info!("Starting Pocket Relay v{}", env::VERSION);

    let db = database::connect().await?;
    let games = Games::new();
    let matchmaking = Matchmaking::new();
    let retriever = Retriever::new().await;

    let (shutdown_send, shutdown_recv) = broadcast::channel(16);
    let global_state = GlobalState {
        db,
        games,
        matchmaking,
        retriever,
        shutdown: shutdown_recv,
    };
    let global_state = Arc::new(global_state);
    select! {
        result = http::start_server(global_state.clone()) => { result? },
        result = blaze::start_server(global_state) => { result? },
        _ = signal::ctrl_c() => {
            shutdown_send
                .send(())
                .expect("Failed to send shutdown signal");
        }
    }

    Ok(())
}
