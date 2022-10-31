use crate::env;
use actix_web::get;
use actix_web::web::{Json, ServiceConfig};
use serde::Serialize;

/// Function for configuring the services in this route
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(server_details);
}

#[derive(Serialize)]
struct ServerDetails {
    /// The external server address value
    address: String,
    /// The server version
    version: &'static str,
    /// The list of proxy services
    services: Vec<ServiceDetails>,
}

/// Describes the details of a service
#[derive(Serialize)]
struct ServiceDetails {
    name: &'static str,
    port: u16,
    #[serde(rename = "type")]
    ty: ServiceType,
}

/// Describes the type of service
#[derive(Serialize)]
#[allow(unused)]
enum ServiceType {
    /// HTTP Proxy Server
    HTTP,
    /// Blaze Packet Proxy Server
    Blaze,
    /// Direct buffer to buffer server (read -> write)
    DirectBuffer,
}

#[get("/api/server")]
async fn server_details() -> Json<ServerDetails> {
    let ext_host = env::ext_host();
    let main_port = env::main_port();
    let http_port = env::http_port();
    Json(ServerDetails {
        address: ext_host,
        version: env::VERSION,
        services: vec![
            ServiceDetails {
                name: "Main Blaze Server",
                ty: ServiceType::Blaze,
                port: main_port,
            },
            ServiceDetails {
                name: "HTTP Server",
                ty: ServiceType::HTTP,
                port: http_port,
            },
        ],
    })
}
