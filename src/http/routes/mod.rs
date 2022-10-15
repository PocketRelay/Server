use actix_web::web::ServiceConfig;

pub mod server;

pub fn configure(cfg: &mut ServiceConfig) {
    server::configure(cfg);
}