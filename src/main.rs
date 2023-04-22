use axum::Server;
use config::load_config;
use log::{error, info};
use state::GlobalState;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use tokio::{select, signal};
use utils::logging;

mod config;
mod database;
mod middleware;
mod routes;
mod services;
mod session;
mod state;
mod utils;

#[tokio::main]
async fn main() {
    // Load configuration
    let config = load_config().unwrap_or_default();

    let port = config.port;

    // Initialize logging
    logging::setup(config.logging);

    info!("Starting Pocket Relay v{}", state::VERSION);

    // Initialize global state
    GlobalState::init(config).await;

    // Display the connection urls message
    logging::log_connection_urls(port).await;

    info!("Starting Server on (Port: {port})");

    // Create HTTP router and socket address
    let router = routes::router();
    let addr: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port));

    // Create futures for server and shutdown signal
    let server_future = Server::bind(&addr).serve(router.into_make_service());
    let close_future = signal::ctrl_c();

    // Await server termination or shutdown signal
    select! {
       result = server_future => {
        if let Err(err) = result {
            error!("Failed to bind HTTP server (Port: {}): {:?}", port, err);
            panic!();
        }
       }
       // Handle the server being stopped with CTRL+C
       _ = close_future => {}
    }

    info!("Shutting down...");
}
