use std::io;
use actix_web::{App, HttpServer};
use log::info;
mod routes;

pub async fn start_server() -> io::Result<()> {

    let http_port = crate::env::http_port();
    info!("Starting HTTP Server on (0.0.0.0:{http_port})");

    HttpServer::new(|| {
        App::new()
            .configure(routes::configure)
    })
        .bind(("0.0.0.0", http_port))?
        .run()
        .await
}