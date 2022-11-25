use log::info;
use tokio::join;
use utils::net::public_address;

use core::{constants, env, state::GlobalState};

use http_server;
use main_server;
use mitm_server;
use redirector_server;

use dotenvy::dotenv;

mod logging;

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

    if env::from_env(env::MITM_ENABLED) {
        // MITM Mode only requires the Redirector & MITM servers
        join!(
            redirector_server::start_server(),
            mitm_server::start_server()
        );
    } else {
        // Normal mode requires the Redirector, HTTP, and Main servers
        join!(
            redirector_server::start_server(),
            http_server::start_server(),
            main_server::start_server()
        );
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
        if output.len() > 0 {
            output.push_str(", ");
        }

        output.push_str("WAN: ");
        output.push_str(&public_address.to_string());
        if http_port != 80 {
            output.push(':');
            output.push_str(&http_port.to_string());
        }
    }

    if output.len() > 0 {
        output.push_str(", ");
    }

    output.push_str("LOCAL: 127.0.0.1");
    if http_port != 80 {
        output.push(':');
        output.push_str(&http_port.to_string());
    }

    info!("Connection URLS ({output})");
}
