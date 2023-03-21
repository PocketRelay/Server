//! This modules contains routes that handle serving information
//! about the server such as the version and services running

use crate::{
    servers::http::{ext::ErrorStatusCode, middleware::auth::AdminAuth},
    state,
    utils::logging::LOG_FILE_NAME,
};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use database::PlayerRole;
use serde::Serialize;
use thiserror::Error;
use tokio::{fs::read_to_string, io};

/// Router function creates a new router with all the underlying
/// routes for this file.
///
/// Prefix: /api/server
pub fn router() -> Router {
    Router::new()
        .route("/", get(server_details))
        .route("/log", get(get_log))
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
