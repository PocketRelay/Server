use actix_web::get;
use actix_web::web::{Json, ServiceConfig};
use serde::Serialize;
use crate::env;

/// Function for configuring the services in this route
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(server_details);
}

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
    name: &'static str,
    port: u16,
    #[serde(rename = "type")]
    ty: ServiceType
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
    DirectBuffer
}

#[get("/api/server")]
async fn server_details() -> Json<ServerDetails> {
    let main_port = env::main_port();
    let http_port = env::http_port();
    Json(ServerDetails {
        version: env::VERSION,
        services: vec![
            ServiceDetails {
                name: "Main Blaze Server",
                ty: ServiceType::Blaze,
                port: main_port
            },
            ServiceDetails {
                name: "HTTP Server",
                ty: ServiceType::HTTP,
                port: http_port
            }
        ]
    })
}
