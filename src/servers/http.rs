use std::net::SocketAddr;

use crate::{env, state::GlobalState};
use axum::Server;
use log::{error, info};
use tokio::select;

mod middleware;
mod routes;
mod stores;

/// Starts the HTTP server
pub async fn start_server() {
    let port = env::from_env(env::HTTP_PORT);
    info!("Starting HTTP Server on (Port: {port})");

    let mut shutdown = GlobalState::shutdown();
    let router = routes::router();
    let addr: SocketAddr = ([0, 0, 0, 0], port).into();
    let future = Server::bind(&addr).serve(router.into_make_service());
    select! {
        result = future => {
            if let Err(err) = result {
                error!("Failed to bind HTTP server (Port: {}): {:?}", port, err);
                panic!();
            }
        }
        // Shutdown hook to ensure we don't keep trying to process after shutdown
        _ = shutdown.changed() => { }
    };
}
