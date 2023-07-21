//! This modules contains routes that handle serving information
//! about the server such as the version and services running

use crate::{
    database::entities::players::PlayerRole,
    middleware::{auth::AdminAuth, blaze_upgrade::BlazeUpgrade},
    session::Session,
    state,
    utils::logging::LOG_FILE_NAME,
};
use axum::{
    body::Empty,
    extract::ConnectInfo,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use blaze_pk::packet::PacketCodec;
use interlink::service::Service;
use log::{debug, error};
use serde::{Deserialize, Serialize};
use std::{
    net::SocketAddr,
    sync::atomic::{AtomicU32, Ordering},
};
use tokio::{fs::read_to_string, io::split};
use tokio_util::codec::{FramedRead, FramedWrite};

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
        version: state::VERSION,
    })
}

/// GET /api/server/upgrade
///
/// Handles upgrading connections from the Pocket Relay Client tool
/// from HTTP over to the Blaze protocol for proxing the game traffic
/// as blaze sessions using HTTP Upgrade
pub async fn upgrade(
    ConnectInfo(socket_addr): ConnectInfo<SocketAddr>,
    upgrade: BlazeUpgrade,
) -> Response {
    tokio::spawn(async move {
        let socket = match upgrade.upgrade().await {
            Ok(value) => value,
            Err(err) => {
                error!("Failed to upgrade blaze socket: {}", err);
                return;
            }
        };
        Session::create(|ctx| {
            // Obtain a session ID
            let session_id = SESSION_IDS.fetch_add(1, Ordering::AcqRel);

            // Attach reader and writers to the session context
            let (read, write) = split(socket.upgrade);
            let read = FramedRead::new(read, PacketCodec);
            let write = FramedWrite::new(write, PacketCodec);

            ctx.attach_stream(read, true);
            let writer = ctx.attach_sink(write);

            Session::new(session_id, socket.host_target, writer, socket_addr)
        });
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
pub async fn get_log(auth: AdminAuth) -> Result<String, StatusCode> {
    let auth = auth.into_inner();
    if auth.role < PlayerRole::SuperAdmin {
        return Err(StatusCode::UNAUTHORIZED);
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
