use crate::env;
use crate::GlobalState;
use actix_web::middleware::Logger;
use actix_web::web::Data;
use actix_web::{App, HttpServer};
use log::info;
use std::io;
use std::sync::Arc;

mod routes;

pub async fn start_server(global: Arc<GlobalState>) -> io::Result<()> {
    let http_port = env::u16_env(env::HTTP_PORT);
    info!("Starting HTTP Server on (0.0.0.0:{http_port})");

    HttpServer::new(move || {
        App::new()
            .app_data(Data::from(global.clone()))
            .wrap(Logger::default())
            .configure(routes::configure)
    })
    .bind(("0.0.0.0", http_port))?
    .run()
    .await
}
