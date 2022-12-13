use crate::stores::token::TokenStore;
use actix_cors::Cors;
use actix_web::middleware::Logger;
use actix_web::{App, HttpServer};
use core::env;
use log::{error, info};
use std::sync::Arc;

mod middleware;
mod routes;
mod stores;

/// Starts the HTTP server
pub async fn start_server() {
    let port = env::from_env(env::HTTP_PORT);
    info!("Starting HTTP Server on (Port: {port})");

    let token_store = Arc::new(TokenStore::default());

    let result = HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(Cors::permissive())
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
