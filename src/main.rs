use dotenvy::dotenv;
use log::info;
use state::GlobalState;
use utils::net::public_address;

mod blaze;
mod constants;
mod env;
mod game;
mod leaderboard;
mod logging;
mod retriever;
mod servers;
mod state;

use servers::{http, main, mitm, redirector};

#[tokio::main]
async fn main() {
    // Load environment variables from nearest .env
    dotenv().ok();

    // Initialize logging
    logging::setup();

    info!("Starting Pocket Relay v{}", constants::VERSION);

    log_connection_urls().await;

    // Initialize global state
    GlobalState::init().await;

    // Spawn redirector in its own task
    tokio::spawn(redirector::start_server());

    if env::from_env(env::MITM_ENABLED) {
        // Start the MITM server
        mitm::start_server().await;
    } else {
        // Spawn the Main server in its own task
        tokio::spawn(main::start_server());
        // Start the HTTP server
        http::start_server().await;
    }
}

/// Prints a list of possible urls that can be used to connect to
/// this Pocket relay server
async fn log_connection_urls() {
    let http_port = env::from_env(env::HTTP_PORT);
    let mut output = String::new();
    if let Ok(local_address) = local_ip_address::local_ip() {
        output.push_str("LAN: ");
        output.push_str(&local_address.to_string());
        if http_port != 80 {
            output.push(':');
            output.push_str(&http_port.to_string());
        }
    }
    if let Some(public_address) = public_address().await {
        if !output.is_empty() {
            output.push_str(", ");
        }

        output.push_str("WAN: ");
        output.push_str(&public_address);
        if http_port != 80 {
            output.push(':');
            output.push_str(&http_port.to_string());
        }
    }

    if !output.is_empty() {
        output.push_str(", ");
    }

    output.push_str("LOCAL: 127.0.0.1");
    if http_port != 80 {
        output.push(':');
        output.push_str(&http_port.to_string());
    }

    info!("Connection URLS ({output})");
}
