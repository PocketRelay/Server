//! This modules contains routes that handle serving information
//! about the server such as the version and services running

use crate::{
    servers::http::{ext::ErrorStatusCode, middleware::auth::AdminAuth},
    utils::env,
};
use axum::{
    extract::Path,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use database::PlayerRole;
use serde::Serialize;
use thiserror::Error;
use tokio::{
    fs::{read_dir, read_to_string},
    io,
};

/// Router function creates a new router with all the underlying
/// routes for this file.
///
/// Prefix: /api/server
pub fn router() -> Router {
    Router::new()
        .route("/", get(server_details))
        .route("/logs", get(list_logs))
        .route("/logs/:name", get(get_log))
}

/// Response detailing the information about this Pocket Relay server
/// contains the version information as well as the server information
#[derive(Serialize)]
struct ServerDetails {
    /// Identifier used to ensure the server is a Pocket Relay server
    ident: &'static str,
    /// The server version
    version: &'static str,
    /// Git branch
    branch: &'static str,
    /// Git SHA
    hash: &'static str,
}

/// Route for retrieving the server details responds with
/// the list of servers and server version.
async fn server_details() -> Json<ServerDetails> {
    Json(ServerDetails {
        ident: "POCKET_RELAY_SERVER",
        version: env::VERSION,
        branch: env::GIT_BRANCH,
        hash: env::GIT_SHA_SHORT,
    })
}

#[derive(Serialize)]
struct LogsList {
    files: Vec<String>,
}

#[derive(Debug, Error)]
enum LogsError {
    #[error("{0}")]
    IO(#[from] io::Error),
    #[error("Invalid log path")]
    InvalidPath,
    #[error("Invalid permission")]
    InvalidPermission,
}

async fn list_logs(auth: AdminAuth) -> Result<Json<LogsList>, LogsError> {
    let auth = auth.into_inner();
    if auth.role < PlayerRole::SuperAdmin {
        return Err(LogsError::InvalidPermission);
    }

    let path = env::env(env::LOGGING_DIR);
    let mut read_dir = read_dir(&path).await?;

    let mut files = Vec::new();

    while let Some(file) = read_dir.next_entry().await? {
        let name = file.file_name().to_string_lossy().to_string();
        let file_type = file.file_type().await?;
        if !file_type.is_file() || !name.ends_with(".log") {
            continue;
        }
        files.push(name);
    }

    Ok(Json(LogsList { files }))
}

async fn get_log(Path(name): Path<String>, auth: AdminAuth) -> Result<String, LogsError> {
    let auth = auth.into_inner();
    if auth.role < PlayerRole::SuperAdmin {
        return Err(LogsError::InvalidPermission);
    }

    let logging_root = env::env(env::LOGGING_DIR);

    let path = std::path::Path::new(&logging_root).join(name);

    if !path.starts_with(logging_root) {
        return Err(LogsError::InvalidPath);
    }

    let file = read_to_string(path).await?;
    Ok(file)
}

/// Error status code implementation for the different error
/// status codes of each error
impl ErrorStatusCode for LogsError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidPermission => StatusCode::UNAUTHORIZED,
            Self::InvalidPath => StatusCode::BAD_REQUEST,
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
