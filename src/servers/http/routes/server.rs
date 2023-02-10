//! This modules contains routes that handle serving information
//! about the server such as the version and services running

use crate::utils::env;
use axum::{routing::get, Json, Router};
use serde::Serialize;

/// Router function creates a new router with all the underlying
/// routes for this file.
///
/// Prefix: /api/server
pub fn router() -> Router {
    Router::new().route("/", get(server_details))
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
        version: env::VERSION,
    })
}
