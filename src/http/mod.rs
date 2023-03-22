use std::net::SocketAddr;

use crate::state::GlobalState;
use axum::Server;
use log::{error, info};

mod ext;
mod middleware;
mod routes;

/// Starts the HTTP server
pub async fn start_server() {
    let config = GlobalState::config();
    let port = config.port;
    info!("Starting HTTP Server on (Port: {port})");

    let router = routes::router();
    let addr: SocketAddr = ([0, 0, 0, 0], port).into();
    if let Err(err) = Server::bind(&addr).serve(router.into_make_service()).await {
        error!("Failed to bind HTTP server (Port: {}): {:?}", port, err);
        panic!();
    }
}
