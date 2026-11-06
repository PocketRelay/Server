#![warn(unused_crate_dependencies)]

use crate::{
    config::VERSION,
    database::{connect_database, data_mover::copy_database},
    services::{retriever::Retriever, sessions::Sessions, tunnel::TunnelService},
    utils::signing::SigningKey,
};
use axum::{self, Extension};
use config::{TunnelConfig, load_config};
use log::{debug, error, info};
use pocket_relay as _;
use services::{
    game::{matchmaking::Matchmaking, store::Games},
    tunnel::{tunnel_keep_alive, udp_tunnel::start_udp_tunnel},
};
use std::{env::args, net::SocketAddr, process::exit, sync::Arc};
use tokio::{join, net::TcpListener, signal};
use utils::logging;

pub mod config;
pub mod database;
pub mod middleware;
pub mod routes;
pub mod services;
pub mod session;
pub mod utils;

#[tokio::main]
async fn main() {
    // Load configuration
    let mut config = load_config().unwrap_or_default();

    // Initialize logging
    logging::setup(config.logging);

    let args: Vec<String> = args().collect();
    let command = args.get(1);
    if let Some(command) = command {
        match command.as_str() {
            // Command to handle migrating from the local sqlite database
            "migrate-sqlite-to-database" => {
                let db_config = &config.database;

                if db_config.connection_url.is_none() {
                    log::error!(
                        "missing database connection_url config, no database target to migrate to"
                    );
                    exit(1);
                }

                // Recreate the database configuration dropping the connection URL so
                // that we connect to the sqlite database while respecting other settings
                let mut sqlite_config = db_config.clone();
                sqlite_config.connection_url = None;

                log::info!("connecting to sqlite database");
                let sqlite_database = connect_database(&sqlite_config).await;

                log::info!("connecting to destination database");
                let dest_database = connect_database(db_config).await;

                if let Err(error) = copy_database(&sqlite_database, &dest_database).await {
                    log::error!("failed to copy database: {error}\n\ndebug: {error:?}");
                    exit(1)
                }

                return;
            }

            unknown => {
                log::warn!("unknown command \"{unknown}\"");
            }
        }
    }

    // Create the server socket address while the port is still available
    let addr: SocketAddr = SocketAddr::new(config.host, config.port);

    debug!("QoS server: {:?}", config.qos);

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
                error!("failed to start UDP tunnel server: {err}");

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

    info!("Starting server on {addr} (v{VERSION})");

    // Start the TCP listener
    let listener = match TcpListener::bind(addr).await {
        Ok(value) => value,
        Err(err) => {
            error!("Failed to bind HTTP server pm {addr}: {err:?}");
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
        error!("Error within HTTP server {err:?}");
    }
}
