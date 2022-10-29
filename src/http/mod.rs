use std::io;
use std::sync::Arc;
use actix_web::{App, HttpServer};
use actix_web::middleware::Logger;
use actix_web::web::Data;
use log::info;
use crate::GlobalState;

mod routes;

pub async fn start_server(global: Arc<GlobalState>) -> io::Result<()> {

    let http_port = crate::env::http_port();
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