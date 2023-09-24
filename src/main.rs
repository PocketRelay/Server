use crate::{
    config::{RuntimeConfig, VERSION},
    services::{
        game::manager::GameManager, leaderboard::Leaderboard, retriever::Retriever,
        sessions::Sessions,
    },
    utils::signing::SigningKey,
};
use axum::{Extension, Server};
use config::load_config;
use log::{error, info, LevelFilter};
use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};
use tokio::{join, signal};
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
        qos: config.qos,
    };

    // This step may take longer than expected so its spawned instead of joined
    tokio::spawn(logging::log_connection_urls(config.port));

    let (db, retriever, signing_key) = join!(
        database::init(&runtime_config),
        Retriever::start(config.retriever),
        SigningKey::global()
    );

    let game_manager = Arc::new(GameManager::new());
    let leaderboard = Arc::new(Leaderboard::new());
    let sessions = Arc::new(Sessions::new(signing_key));
    let config = Arc::new(runtime_config);
    let retriever = Arc::new(retriever);

    // Initialize session router
    let mut router = session::routes::router();

    router.add_extension(db.clone());
    router.add_extension(config.clone());
    router.add_extension(retriever);
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

    info!("Starting server on {} (v{})", addr, VERSION);

    if let Err(err) = Server::bind(&addr)
        .serve(router)
        .with_graceful_shutdown(async move {
            _ = signal::ctrl_c().await;
        })
        .await
    {
        error!("Failed to bind HTTP server on {}: {:?}", addr, err);
    }
}
