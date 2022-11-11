use actix_web::middleware::Logger;
use actix_web::web::Data;
use actix_web::{App, HttpServer};
use core::{env, GlobalStateArc};
use log::{error, info};

mod routes;

/// Starts the HTTP server using the provided global state
/// which is cloned for use as app data on the server.
///
/// `global` The global state
pub async fn start_server(global: GlobalStateArc) {
    let port = env::u16_env(env::HTTP_PORT);
    info!("Starting HTTP Server on (Port: {port})");

    let result = HttpServer::new(move || {
        App::new()
            .app_data(Data::from(global.clone()))
            .wrap(Logger::default())
            .configure(routes::configure)
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
