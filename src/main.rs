use dotenvy::dotenv;
use log::info;
use servers::*;
use state::GlobalState;
use tokio::signal;
use utils::{env, logging};

mod config;
mod servers;
mod services;
mod state;
mod utils;

fn main() {
    // Create the tokio runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed building the tokio Runtime");

    // Load environment variables from nearest .env
    dotenv().ok();

    // Initialize logging
    logging::setup();

    info!("Starting Pocket Relay v{}", env::VERSION);

    // Initialize global state
    runtime.block_on(GlobalState::init());

    // Display the connection urls message
    runtime.block_on(logging::log_connection_urls());

    // Spawn redirector in its own task
    runtime.spawn(redirector::start_server());
    // Spawn QOS server in its own task
    runtime.spawn(qos::start_server());
    // Spawn the HTTP server in its own task
    runtime.spawn(http::start_server());
    // Spawn the Main server in its own task
    runtime.spawn(main::start_server());
    // Spawn the Telemetry server in its own task
    runtime.spawn(telemetry::start_server());

    // Block until shutdown is recieved
    runtime.block_on(signal::ctrl_c()).ok();

    info!("Shutting down...");
}
