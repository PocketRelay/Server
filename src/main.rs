mod blaze;
mod database;
mod env;
mod game;
mod http;
mod redirector;
mod retriever;

use crate::game::Games;
use dotenvy::dotenv;
use game::matchmaking::Matchmaking;
use log::{error, info};
use retriever::Retriever;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::{self, select, signal};

/// Global state that is shared throughout the application
pub struct GlobalState {
    pub games: Games,
    pub matchmaking: Matchmaking,
    pub db: DatabaseConnection,
    pub retriever: Option<Retriever>,
    pub shutdown: watch::Receiver<()>,
}

pub type GlobalStateArc = Arc<GlobalState>;

#[tokio::main]
async fn main() {
    dotenv().ok();

    {
        let logging_level = env::logging_level();
        let logging_path = env::str_env(env::LOGGING_DIR);
        utils::logging::init_logger(logging_level, logging_path);
    }

    info!("Starting Pocket Relay v{}", env::VERSION);

    let db = match database::connect().await {
        Ok(value) => value,
        Err(err) => {
            error!("Unable to connect to database: {:?}", err);
            panic!();
        }
    };

    let games = Games::new();
    let matchmaking = Matchmaking::new();
    let retriever = Retriever::new().await;

    let (shutdown_send, shutdown_recv) = watch::channel(());
    let global_state = GlobalState {
        db,
        games,
        matchmaking,
        retriever,
        shutdown: shutdown_recv,
    };
    let global_state = Arc::new(global_state);
    select! {
        _ = http::start_server(global_state.clone()) => { },
        _ = redirector::start_server(global_state.clone()) => { },
        _ = blaze::start_server(global_state) => { },
        _ = signal::ctrl_c() => {
            shutdown_send
                .send(())
                .expect("Failed to send shutdown signal");
        }
    };
}
