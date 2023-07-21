use axum::Server;
use config::load_config;
use log::{error, info};
use state::App;
use std::net::{Ipv4Addr, SocketAddr};
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

    // Initialize logging
    logging::setup(config.logging);

    // Create the server socket address while the port is still available
    let addr: SocketAddr = (Ipv4Addr::UNSPECIFIED, config.port).into();

    // Initialize global state
    App::init(config).await;

    // Create the HTTP router
    let router = routes::router().into_make_service_with_connect_info::<SocketAddr>();

    // Create futures for server and shutdown signal
    let server_future = Server::bind(&addr).serve(router);
    let close_future = signal::ctrl_c();

    info!("Started server on {} (v{})", addr, state::VERSION);

    // Await server termination or shutdown signal
    select! {
       result = server_future => {
        if let Err(err) = result {
            error!("Failed to bind HTTP server on {}: {:?}", addr, err);
            panic!();
        }
       }
       // Handle the server being stopped with CTRL+C
       _ = close_future => {}
    }
}
