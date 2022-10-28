use actix_web::web::ServiceConfig;

mod server;
mod public;
mod gaw;

pub fn configure(cfg: &mut ServiceConfig) {
    server::configure(cfg);
    public::configure(cfg);
    gaw::configure(cfg);
}