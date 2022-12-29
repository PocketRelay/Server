use std::net::SocketAddr;

use crate::env;
use axum::Server;
use log::{error, info};

mod ext;
mod middleware;
mod routes;
mod stores;

/// Starts the HTTP server
pub async fn start_server() {
    let port = env::from_env(env::HTTP_PORT);
    info!("Starting HTTP Server on (Port: {port})");

    let router = routes::router();
    let addr: SocketAddr = ([0, 0, 0, 0], port).into();
    if let Err(err) = Server::bind(&addr).serve(router.into_make_service()).await {
        error!("Failed to bind HTTP server (Port: {}): {:?}", port, err);
        panic!();
    }
}
