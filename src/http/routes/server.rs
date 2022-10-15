use actix_web::{HttpResponse, Responder, get};
use actix_web::web::{Json, ServiceConfig};
use serde::Serialize;
use crate::env;

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(server_details);
}

#[derive(Serialize)]
pub struct ServerDetails {
    version: &'static str,
    main: u16,
    http: u16
}

#[get("/api/server")]
pub async fn server_details() -> Json<ServerDetails> {
    let main_port = env::main_port();
    let http_port = env::http_port();
    Json(ServerDetails {
        version: env::VERSION,
        main: main_port,
        http: http_port
    })
}
