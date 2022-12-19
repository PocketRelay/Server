//! This modules contains routes that handle serving information
//! about the server such as the version and services running

use crate::utils::{constants, env};
use axum::{routing::get, Json, Router};
use serde::Serialize;

/// Function for adding all the routes in this file to
/// the provided router
///
/// `router` The route to add to
pub fn route(router: Router) -> Router {
    router.route("/api/server", get(server_details))
}

/// Response detailing the information about this Pocket Relay server
/// contains the version information as well as the server information
#[derive(Serialize)]
struct ServerDetails {
    /// The server version
    version: &'static str,
    /// The list of proxy services
    services: Vec<ServiceDetails>,
}

/// Describes the details of a service
#[derive(Serialize)]
struct ServiceDetails {
    /// The name of the server
    name: &'static str,
    /// The port of the service
    port: u16,
    /// The type of service it is
    #[serde(rename = "type")]
    ty: ServiceType,
}

/// Describes the type of service
#[derive(Serialize)]
#[allow(unused)]
enum ServiceType {
    /// HTTP Server
    Http,
    /// Blaze Packet Server
    Blaze,
    /// Blaze SSL Packet Server
    BlazeSecure,
    /// Direct buffer to buffer server (read -> write)
    DirectBuffer,
}

/// Route for retrieving the server details responds with
/// the list of servers and server version.
async fn server_details() -> Json<ServerDetails> {
    let redirector_port = env::from_env(env::REDIRECTOR_PORT);
    let main_port = env::from_env(env::MAIN_PORT);
    let http_port = env::from_env(env::HTTP_PORT);
    Json(ServerDetails {
        version: constants::VERSION,
        services: vec![
            ServiceDetails {
                name: "Redirector Server",
                ty: ServiceType::BlazeSecure,
                port: redirector_port,
            },
            ServiceDetails {
                name: "Main Server",
                ty: ServiceType::Blaze,
                port: main_port,
            },
            ServiceDetails {
                name: "HTTP Server",
                ty: ServiceType::Http,
                port: http_port,
            },
        ],
    })
}