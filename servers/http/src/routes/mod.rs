use actix_web::web::ServiceConfig;

mod gaw;
mod public;
mod server;

pub fn configure(cfg: &mut ServiceConfig) {
    server::configure(cfg);
    public::configure(cfg);
    gaw::configure(cfg);
}
