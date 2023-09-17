//! This modules contains routes that handle serving information
//! about the server such as the version and services running

use crate::{
    config::{RuntimeConfig, VERSION},
    database::entities::players::PlayerRole,
    middleware::{
        auth::AdminAuth,
        blaze_upgrade::{BlazeSocket, BlazeUpgrade},
        ip_address::IpAddress,
    },
    services::sessions::Sessions,
    session::{router::BlazeRouter, Session},
    utils::logging::LOG_FILE_NAME,
};
use axum::{
    body::Empty,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Extension, Json,
};
use log::{debug, error};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use tokio::fs::read_to_string;

static SESSION_IDS: AtomicU32 = AtomicU32::new(1);

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
    upgrade: BlazeUpgrade,
) -> Response {
    // TODO: Socket address extraction for forwarded reverse proxy

    tokio::spawn(async move {
        let BlazeSocket(upgrade) = match upgrade.upgrade().await {
            Ok(value) => value,
            Err(err) => {
                error!("Failed to upgrade blaze socket: {}", err);
                return;
            }
        };

        // Obtain a session ID
        let session_id = SESSION_IDS.fetch_add(1, Ordering::AcqRel);

        Session::start(session_id, upgrade, addr, router, sessions);
    });

    let mut response = Empty::new().into_response();
    // Use the switching protocols status code
    *response.status_mut() = StatusCode::SWITCHING_PROTOCOLS;

    let headers = response.headers_mut();
    // Add the upgraidng headers
    headers.insert(header::CONNECTION, HeaderValue::from_static("upgrade"));
    headers.insert(header::UPGRADE, HeaderValue::from_static("blaze"));

    response
}

/// GET /api/server/log
///
/// Handles loading and responding with the server log file
/// contents for the log section on the super admin portion
/// of the dashboard
pub async fn get_log(AdminAuth(auth): AdminAuth) -> Result<String, StatusCode> {
    if auth.role < PlayerRole::SuperAdmin {
        return Err(StatusCode::FORBIDDEN);
    }
    let path = std::path::Path::new(LOG_FILE_NAME);
    read_to_string(path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
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
