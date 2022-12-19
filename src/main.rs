use dotenvy::dotenv;
use log::info;
use servers::*;
use state::GlobalState;
use utils::{constants::VERSION, env, logging};

mod blaze;
mod game;
mod leaderboard;
mod retriever;
mod servers;
mod state;
mod utils;

#[tokio::main]
async fn main() {
    // Load environment variables from nearest .env
    dotenv().ok();

    // Initialize logging
    logging::setup();

    info!("Starting Pocket Relay v{}", VERSION);

    logging::log_connection_urls().await;

    // Initialize global state
    GlobalState::init().await;

    // Spawn redirector in its own task
    tokio::spawn(redirector::start_server());

    if env::from_env(env::MITM_ENABLED) {
        // Start the MITM server
        mitm::start_server().await;
    } else {
        // Spawn the Main server in its own task
        tokio::spawn(http::start_server());
        // Start the HTTP server
        main::start_server().await;
    }
}
