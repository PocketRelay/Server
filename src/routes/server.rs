//! This modules contains routes that handle serving information
//! about the server such as the version and services running

use crate::{
    config::{RuntimeConfig, VERSION},
    database::entities::players::PlayerRole,
    middleware::{auth::AdminAuth, ip_address::IpAddress, upgrade::Upgrade},
    services::sessions::Sessions,
    session::{router::BlazeRouter, Session},
    utils::logging::LOG_FILE_NAME,
};
use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Extension, Json,
};
use hyper::upgrade::OnUpgrade;
use log::{debug, error};
use serde::{Deserialize, Serialize};
use std::{net::Ipv4Addr, sync::Arc};
use tokio::fs::{read_to_string, OpenOptions};

/// Response detailing the information about this Pocket Relay server
/// contains the version information as well as the server information
#[derive(Serialize)]
pub struct ServerDetails {
    /// Identifier used to ensure the server is a Pocket Relay server
    ident: &'static str,
    /// The server version
    version: &'static str,
}

/// GET /api/server
///
/// Handles providing the server details. The Pocket Relay client tool
/// uses this endpoint to validate that the provided host is a valid
/// Pocket Relay server.
pub async fn server_details() -> Json<ServerDetails> {
    Json(ServerDetails {
        ident: "POCKET_RELAY_SERVER",
        version: VERSION,
    })
}

/// Response sent to dashboard clients containing configuration
/// information about the dashboard
#[derive(Serialize)]
pub struct DashboardDetails {
    pub disable_registration: bool,
}

/// GET /api/server/dashboard
///
/// Handles providing the server details. The Pocket Relay client tool
/// uses this endpoint to validate that the provided host is a valid
/// Pocket Relay server.
pub async fn dashboard_details(
    Extension(config): Extension<Arc<RuntimeConfig>>,
) -> Json<DashboardDetails> {
    Json(DashboardDetails {
        disable_registration: config.dashboard.disable_registration,
    })
}

/// GET /api/server/upgrade
///
/// Handles upgrading connections from the Pocket Relay Client tool
/// from HTTP over to the Blaze protocol for proxing the game traffic
/// as blaze sessions using HTTP Upgrade
pub async fn upgrade(
    IpAddress(addr): IpAddress,
    Extension(router): Extension<Arc<BlazeRouter>>,
    Extension(sessions): Extension<Arc<Sessions>>,
    Upgrade(upgrade): Upgrade,
) -> Response {
    // Spawn the upgrading process to its own task
    tokio::spawn(handle_upgrade(upgrade, addr, router, sessions));

    // Let the client know to upgrade its connection
    (
        // Switching protocols status code
        StatusCode::SWITCHING_PROTOCOLS,
        // Headers required for upgrading
        [(header::CONNECTION, "upgrade"), (header::UPGRADE, "blaze")],
    )
        .into_response()
}

/// Handles upgrading a connection and starting a new session
/// from the connection
pub async fn handle_upgrade(
    upgrade: OnUpgrade,
    addr: Ipv4Addr,
    router: Arc<BlazeRouter>,
    sessions: Arc<Sessions>,
) {
    let upgraded = match upgrade.await {
        Ok(upgraded) => upgraded,
        Err(err) => {
            error!("Failed to upgrade client connection: {}", err);
            return;
        }
    };

    Session::start(upgraded, addr, router, sessions).await;
}

/// GET /api/server/log
///
/// Responds with the server log file contents
///
/// Requires super admin authentication
pub async fn get_log(AdminAuth(auth): AdminAuth) -> Result<String, StatusCode> {
    if auth.role < PlayerRole::SuperAdmin {
        return Err(StatusCode::FORBIDDEN);
    }
    let path = std::path::Path::new(LOG_FILE_NAME);
    read_to_string(path).await.map_err(|err| {
        error!("Failed to read server log file: {}", err);
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

/// DELETE /api/server/log
///
/// Truncates the server log file, useful for long log files that
/// are starting to take up lots of space or have out-served their
/// usefulness
///
/// Requires super admin authentication
pub async fn clear_log(AdminAuth(auth): AdminAuth) -> Result<(), StatusCode> {
    if auth.role < PlayerRole::SuperAdmin {
        return Err(StatusCode::FORBIDDEN);
    }

    let path = std::path::Path::new(LOG_FILE_NAME);

    // Open the file
    let file = OpenOptions::new()
        .write(true)
        .open(path)
        .await
        .map_err(|err| {
            error!("Failed to open server log file: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Truncate the file
    file.set_len(0).await.map_err(|err| {
        error!("Failed to truncate server log file: {}", err);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(())
}

/// Structure of a telemetry message coming from a client
#[derive(Debug, Deserialize)]
pub struct TelemetryMessage {
    /// The telemetry message values
    pub values: Vec<(String, String)>,
}

/// GET /api/server/telemetry
///
/// Handles the incoming telemetry messages recieved
/// from Pocket Relay clients
pub async fn submit_telemetry(Json(data): Json<TelemetryMessage>) -> StatusCode {
    debug!("[TELEMETRY] {:?}", data);
    StatusCode::OK
}
