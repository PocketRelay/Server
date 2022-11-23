use actix_web::middleware::Logger;

use actix_web::{App, HttpServer};
use core::env;
use log::{error, info};
use std::sync::Arc;

use crate::stores::token::TokenStore;

mod middleware;
mod routes;
mod stores;

/// Starts the HTTP server using the provided global state
/// which is cloned for use as app data on the server.
pub async fn start_server() {
    let port = env::from_env(env::HTTP_PORT);
    info!("Starting HTTP Server on (Port: {port})");

    let token_store = Arc::new(TokenStore::new());

    let result = HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .configure(|cfg| routes::configure(cfg, token_store.clone()))
    })
    .bind(("0.0.0.0", port));
    match result {
        Ok(value) => {
            if let Err(err) = value.run().await {
                error!("Error while running HTTP server: {:?}", err);
                panic!();
            }
        }
        Err(err) => {
            error!("Failed to bind HTTP server (Port: {}): {:?}", port, err);
            panic!();
        }
    }
}
