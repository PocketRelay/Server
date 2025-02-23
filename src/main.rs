#![warn(unused_crate_dependencies)]

use crate::{
    config::VERSION,
    services::{retriever::Retriever, sessions::Sessions, tunnel::TunnelService},
    utils::signing::SigningKey,
};
use axum::{self, Extension};
use config::{load_config, TunnelConfig};
use log::{debug, error, info, LevelFilter};
use services::{
    game::{matchmaking::Matchmaking, store::Games},
    tunnel::{tunnel_keep_alive, udp_tunnel::start_udp_tunnel},
};
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
    let mut config = load_config().unwrap_or_default();

    // Initialize logging
    logging::setup(config.logging);

    if config.logging == LevelFilter::Debug {
        utils::components::initialize();
    }

    // Create the server socket address while the port is still available
    let addr: SocketAddr = SocketAddr::new(config.host, config.port);

    debug!("QoS server: {:?}", &config.qos);

    // This step may take longer than expected so its spawned instead of joined
    tokio::spawn(logging::log_connection_urls(config.port));

    let (db, retriever, signing_key) = join!(
        database::init(&config),
        Retriever::start(&config.retriever),
        SigningKey::global(),
    );

    let (tunnel_service, udp_forward_rx) = TunnelService::new();
    let tunnel_service = Arc::new(tunnel_service);

    let sessions = Arc::new(Sessions::new(signing_key));
    let games = Arc::new(Games::default());
    let matchmaking = Arc::new(Matchmaking::default());
    let retriever = Arc::new(retriever);

    // Start tunnel if not disabled
    if !matches!(config.tunnel, TunnelConfig::Disabled) {
        tokio::spawn(tunnel_keep_alive(tunnel_service.clone()));

        // Start UDP tunnel if enabled
        if config.udp_tunnel.enabled {
            // Create tunnel server socket address
            let tunnel_addr: SocketAddr = SocketAddr::new(config.host, config.udp_tunnel.port);

            // Start the tunnel service server
            if let Err(err) = start_udp_tunnel(
                tunnel_addr,
                tunnel_service.clone(),
                sessions.clone(),
                udp_forward_rx,
            )
            .await
            {
                error!("failed to start UDP tunnel server: {}", err);

                // Disable failed UDP tunnel
                config.udp_tunnel.enabled = false;
            }
        }
    }

    let config = Arc::new(config);

    // Initialize session router
    let blaze_router = session::routes::router()
        .extension(db.clone())
        .extension(config.clone())
        .extension(games.clone())
        .extension(sessions.clone())
        .extension(tunnel_service.clone())
        .extension(matchmaking)
        .extension(retriever)
        .build();

    // Create the HTTP router
    let router = routes::router()
        // Apply data extensions
        .layer(Extension(db))
        .layer(Extension(config))
        .layer(Extension(blaze_router))
        .layer(Extension(games))
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
