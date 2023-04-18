//! This modules contains routes that handle serving information
//! about the server such as the version and services running

use crate::{
    database::PlayerRole,
    middleware::{auth::AdminAuth, blaze_upgrade::BlazeUpgrade},
    session::Session,
    state,
    utils::logging::LOG_FILE_NAME,
};
use axum::{
    body::BoxBody,
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use blaze_pk::packet::PacketCodec;
use hyper::header;
use interlink::service::Service;
use log::{debug, error};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use thiserror::Error;
use tokio::{
    fs::read_to_string,
    io::{self, split},
};
use tokio_util::codec::{FramedRead, FramedWrite};

/// Router function creates a new router with all the underlying
/// routes for this file.
///
/// Prefix: /api/server
pub fn router() -> Router {
    Router::new()
        .route("/", get(server_details))
        .route("/log", get(get_log))
        .route("/upgrade", get(upgrade))
        .route("/telemetry", post(submit_telemetry))
}

static SESSION_IDS: AtomicU32 = AtomicU32::new(1);

/// Route handling upgrading Blaze connections into streams that can
/// be used as blaze sessions
async fn upgrade(upgrade: BlazeUpgrade) -> Result<Response, StatusCode> {
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

            Session::new(session_id, socket.host_target, writer)
        });
    });

    Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .header(header::CONNECTION, HeaderValue::from_static("upgrade"))
        .header(header::UPGRADE, HeaderValue::from_static("blaze"))
        .body(BoxBody::default())
        .map_err(|_| {
            error!("Failed to create upgrade response");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

/// Response detailing the information about this Pocket Relay server
/// contains the version information as well as the server information
#[derive(Serialize)]
struct ServerDetails {
    /// Identifier used to ensure the server is a Pocket Relay server
    ident: &'static str,
    /// The server version
    version: &'static str,
}

/// Route for retrieving the server details responds with
/// the list of servers and server version.
async fn server_details() -> Json<ServerDetails> {
    Json(ServerDetails {
        ident: "POCKET_RELAY_SERVER",
        version: state::VERSION,
    })
}

#[derive(Serialize)]
struct LogsList {
    files: Vec<String>,
}

#[derive(Debug, Error)]
enum LogsError {
    #[error("Failed to read log file")]
    IO(#[from] io::Error),
    #[error("Invalid permission")]
    InvalidPermission,
}

async fn get_log(auth: AdminAuth) -> Result<String, LogsError> {
    let auth = auth.into_inner();
    if auth.role < PlayerRole::SuperAdmin {
        return Err(LogsError::InvalidPermission);
    }
    let path = std::path::Path::new(LOG_FILE_NAME);
    let file = read_to_string(path).await?;
    Ok(file)
}

#[derive(Debug, Deserialize)]
pub struct TelemetryMessage {
    pub values: Vec<(String, String)>,
}

async fn submit_telemetry(Json(data): Json<TelemetryMessage>) -> StatusCode {
    debug!("[TELEMETRY] {:?}", data);
    StatusCode::OK
}

/// IntoResponse implementation for PlayersError to allow it to be
/// used within the result type as a error response
impl IntoResponse for LogsError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::InvalidPermission => StatusCode::UNAUTHORIZED,
            Self::IO(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.to_string()).into_response()
    }
}
