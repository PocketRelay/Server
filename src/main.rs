#![warn(unused_crate_dependencies)]

use crate::{
    config::{RuntimeConfig, VERSION},
    services::{
        game::manager::GameManager, retriever::Retriever, sessions::Sessions, tunnel::TunnelService,
    },
    utils::signing::SigningKey,
};
use axum::{self, Extension};
use config::{load_config, TunnelConfig};
use log::{debug, error, info, LevelFilter};
use services::tunnel::{tunnel_keep_alive, udp_tunnel::start_udp_tunnel};
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
    let tunnel_addr: SocketAddr = SocketAddr::new(config.host, config.udp_tunnel.port);

    // Check if the tunnel is enabled
    let tunnel_enabled: bool = !matches!(config.tunnel, TunnelConfig::Disabled);

    // Config data persisted to runtime
    let runtime_config = RuntimeConfig {
        reverse_proxy: config.reverse_proxy,
        galaxy_at_war: config.galaxy_at_war,
        menu_message: config.menu_message,
        dashboard: config.dashboard,
        qos: config.qos,
        tunnel: config.tunnel,
        api: config.api,
        udp_tunnel: config.udp_tunnel,
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

    let (tunnel_service, udp_forward_rx) = TunnelService::new();
    let tunnel_service = Arc::new(tunnel_service);

    let game_manager = Arc::new(GameManager::new(tunnel_service.clone(), config.clone()));
    let retriever = Arc::new(retriever);

    // Spawn background task to perform keep alive checks on tunnels
    if tunnel_enabled {
        tokio::spawn(tunnel_keep_alive(tunnel_service.clone()));
    }

    // Start the tunnel server (If enabled)
    if tunnel_enabled && config.udp_tunnel.enabled {
        // Start the tunnel service server
        if let Err(err) = start_udp_tunnel(
            tunnel_addr,
            tunnel_service.clone(),
            sessions.clone(),
            udp_forward_rx,
        )
        .await
        {
            error!("failed to start udp tunnel server: {}", err);
        }
    }

    // Initialize session router
    let mut router = session::routes::router();

    router.add_extension(db.clone());
    router.add_extension(config.clone());
    router.add_extension(retriever);
    router.add_extension(game_manager.clone());
    router.add_extension(sessions.clone());

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
