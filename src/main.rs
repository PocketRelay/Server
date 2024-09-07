#![warn(unused_crate_dependencies)]

use crate::{
    config::{RuntimeConfig, VERSION},
    services::{
        game::manager::GameManager, retriever::Retriever, sessions::Sessions, tunnel::TunnelService,
    },
    utils::signing::SigningKey,
};
use axum::{self, Extension};
use config::load_config;
use log::{debug, error, info, LevelFilter};
use services::udp_tunnel::create_tunnel_service;
use std::{net::SocketAddr, sync::Arc};
use tokio::{join, net::TcpListener, signal};
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
    let addr: SocketAddr = SocketAddr::new(config.host, config.port);

    // Create tunnel server socket address
    let tunnel_addr: SocketAddr = SocketAddr::new(config.host, config.tunnel_port);

    // Config data persisted to runtime
    let runtime_config = RuntimeConfig {
        reverse_proxy: config.reverse_proxy,
        galaxy_at_war: config.galaxy_at_war,
        menu_message: config.menu_message,
        dashboard: config.dashboard,
        qos: config.qos,
        tunnel: config.tunnel,
        api: config.api,
        tunnel_port: config.tunnel_port,
        external_tunnel_port: config.external_tunnel_port,
    };

    debug!("QoS server: {:?}", &runtime_config.qos);

    // This step may take longer than expected so its spawned instead of joined
    tokio::spawn(logging::log_connection_urls(config.port));

    let (db, retriever, signing_key) = join!(
        database::init(&runtime_config),
        Retriever::start(config.retriever),
        SigningKey::global(),
    );
    let sessions = Arc::new(Sessions::new(signing_key));
    let config = Arc::new(runtime_config);
    let tunnel_service = Arc::new(TunnelService::default());
    let tunnel_service_v2 = create_tunnel_service(sessions.clone(), tunnel_addr).await;

    let game_manager = Arc::new(GameManager::new(
        tunnel_service.clone(),
        tunnel_service_v2.clone(),
        config.clone(),
    ));
    let retriever = Arc::new(retriever);

    // Initialize session router
    let mut router = session::routes::router();

    router.add_extension(db.clone());
    router.add_extension(config.clone());
    router.add_extension(retriever);
    router.add_extension(game_manager.clone());
    router.add_extension(sessions.clone());
    router.add_extension(tunnel_service_v2.clone());

    let router = router.build();

    // Create the HTTP router
    let router = routes::router()
        // Apply data extensions
        .layer(Extension(db))
        .layer(Extension(config))
        .layer(Extension(router))
        .layer(Extension(game_manager))
        .layer(Extension(sessions))
        .layer(Extension(tunnel_service))
        .layer(Extension(tunnel_service_v2))
        .into_make_service_with_connect_info::<SocketAddr>();

    info!("Starting server on {} (v{})", addr, VERSION);

    // Start the TCP listener
    let listener = match TcpListener::bind(addr).await {
        Ok(value) => value,
        Err(err) => {
            error!("Failed to bind HTTP server pm {}: {:?}", addr, err);
            return;
        }
    };

    // Run the HTTP server
    if let Err(err) = axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            _ = signal::ctrl_c().await;
        })
        .await
    {
        error!("Error within HTTP server {:?}", err);
    }
}
