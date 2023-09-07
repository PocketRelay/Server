use axum::{Extension, Server};
use config::load_config;
use log::{error, info, LevelFilter};
use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};
use tokio::{join, select, signal};
use utils::logging;

use crate::{
    config::{RuntimeConfig, ServicesConfig, VERSION},
    services::Services,
};

mod config;
mod database;
mod middleware;
mod routes;
mod services;
mod session;
mod utils;

#[tokio::main]
async fn main() {
    // Load configuration
    let config = load_config().unwrap_or_default();

    if config.logging == LevelFilter::Debug {
        utils::components::initialize();
    }

    // Initialize logging
    logging::setup(config.logging);

    // Create the server socket address while the port is still available
    let addr: SocketAddr = (Ipv4Addr::UNSPECIFIED, config.port).into();

    // Config data passed onto the services
    let services_config = ServicesConfig {
        retriever: config.retriever,
    };

    // Create menu message
    let menu_message = {
        // Message with server version variable replaced
        let mut message: String = config.menu_message.replace("{v}", VERSION);

        // Line terminator for the end of the message
        message.push(char::from(0x0A));
        message
    };

    // Config data persisted to runtime
    let runtime_config = RuntimeConfig {
        reverse_proxy: config.reverse_proxy,
        galaxy_at_war: config.galaxy_at_war,
        menu_message,
        dashboard: config.dashboard,
    };

    let (db, services, _) = join!(
        // Initialize the database
        database::init(&runtime_config),
        // Initialize the services
        Services::init(services_config),
        // Display the connection urls message
        logging::log_connection_urls(config.port)
    );

    let services = Arc::new(services);
    let config = Arc::new(runtime_config);

    // Initialize session router
    let router = session::routes::router(db.clone(), services.clone(), config.clone());

    // Create the HTTP router
    let router = routes::router()
        // Apply data extensions
        .layer(Extension(db))
        .layer(Extension(services))
        .layer(Extension(config))
        .layer(Extension(router))
        .into_make_service_with_connect_info::<SocketAddr>();

    // Create futures for server and shutdown signal
    let server_future = Server::bind(&addr).serve(router);
    let close_future = signal::ctrl_c();

    info!("Started server on {} (v{})", addr, VERSION);

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
