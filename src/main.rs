use config::load_config;
use log::info;
use state::GlobalState;
use tokio::signal;
use utils::logging;

mod config;
mod http;
mod services;
mod session;
mod state;
mod utils;

fn main() {
    // Create the tokio runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed building the tokio Runtime");

    // Load configuration
    let config = runtime.block_on(load_config()).unwrap_or_default();

    let port = config.port;

    // Initialize logging
    logging::setup(config.logging);

    info!("Starting Pocket Relay v{}", state::VERSION);

    // Initialize global state
    runtime.block_on(GlobalState::init(config));

    // Display the connection urls message
    runtime.block_on(logging::log_connection_urls(port));

    session::init_router();

    // Spawn the HTTP server in its own task
    runtime.spawn(http::start_server(port));

    // Block until shutdown is recieved
    runtime.block_on(signal::ctrl_c()).ok();

    info!("Shutting down...");
}
