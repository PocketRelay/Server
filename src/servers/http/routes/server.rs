//! This modules contains routes that handle serving information
//! about the server such as the version and services running

use std::sync::atomic::{AtomicU32, Ordering};

use crate::{
    servers::{
        http::{
            ext::{blaze_upgrade::BlazeUpgrade, ErrorStatusCode},
            middleware::auth::AdminAuth,
        },
        main::session::Session,
    },
    state,
    utils::logging::LOG_FILE_NAME,
};
use axum::{
    body::{BoxBody, Empty},
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use blaze_pk::packet::PacketCodec;
use database::PlayerRole;
use hyper::header;
use interlink::service::Service;
use log::info;
use serde::Serialize;
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
}

static SESSION_IDS: AtomicU32 = AtomicU32::new(1);

async fn upgrade(upgrade: BlazeUpgrade) -> Response {
    tokio::spawn(async move {
        let socket = upgrade.upgrade().await.unwrap();
        Session::create(|ctx| {
            let session_id = SESSION_IDS.fetch_add(1, Ordering::AcqRel);
            // Attach reader and writers to the session context
            let (read, write) = split(socket.upgrade);
            let read = FramedRead::new(read, PacketCodec);
            let write = FramedWrite::new(write, PacketCodec);

            ctx.attach_stream(read, true);
            let writer = ctx.attach_sink(write);

            Session::new(session_id, socket.socket_addr, writer)
        });
    });

    #[allow(clippy::declare_interior_mutable_const)]
    const UPGRADE: HeaderValue = HeaderValue::from_static("upgrade");
    #[allow(clippy::declare_interior_mutable_const)]
    const BLAZE: HeaderValue = HeaderValue::from_static("blaze");

    Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .header(header::CONNECTION, UPGRADE)
        .header(header::UPGRADE, BLAZE)
        .body(BoxBody::default())
        .unwrap()
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

/// Error status code implementation for the different error
/// status codes of each error
impl ErrorStatusCode for LogsError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidPermission => StatusCode::UNAUTHORIZED,
            Self::IO(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// IntoResponse implementation for PlayersError to allow it to be
/// used within the result type as a error response
impl IntoResponse for LogsError {
    #[inline]
    fn into_response(self) -> Response {
        (self.status_code(), self.to_string()).into_response()
    }
}
