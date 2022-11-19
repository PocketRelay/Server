use actix_web::web::ServiceConfig;

mod games;
mod gaw;
mod players;
mod public;
mod qos;
mod server;

pub fn configure(cfg: &mut ServiceConfig) {
    server::configure(cfg);
    public::configure(cfg);
    gaw::configure(cfg);
    games::configure(cfg);
    players::configure(cfg);
    qos::configure(cfg);
}
