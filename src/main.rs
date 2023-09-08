use crate::{
    config::{RuntimeConfig, VERSION},
    services::{
        game::manager::GameManager, leaderboard::Leaderboard, retriever::Retriever,
        sessions::Sessions,
    },
};
use axum::{Extension, Server};
use config::load_config;
use log::{error, info, LevelFilter};
use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};
use tokio::{join, select, signal};
use utils::logging;

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

    // Config data persisted to runtime
    let runtime_config = RuntimeConfig {
        reverse_proxy: config.reverse_proxy,
        galaxy_at_war: config.galaxy_at_war,
        menu_message: config.menu_message,
        dashboard: config.dashboard,
    };

    tokio::spawn(logging::log_connection_urls(config.port));

    let (db, retriever, sessions) = join!(
        database::init(&runtime_config),
        Retriever::new(config.retriever),
        Sessions::start()
    );
    let game_manager = GameManager::start();
    let leaderboard = Leaderboard::start();
    let config = Arc::new(runtime_config);

    // Initialize session router
    let mut router = session::routes::router();

    router.add_extension(db.clone());
    router.add_extension(config.clone());
    router.add_extension(retriever.clone());
    router.add_extension(game_manager.clone());
    router.add_extension(leaderboard.clone());
    router.add_extension(sessions.clone());

    let router = router.build();

    // Create the HTTP router
    let router = routes::router()
        // Apply data extensions
        .layer(Extension(db))
        .layer(Extension(config))
        .layer(Extension(router))
        .layer(Extension(game_manager))
        .layer(Extension(leaderboard))
        .layer(Extension(sessions))
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
