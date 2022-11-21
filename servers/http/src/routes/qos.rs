use core::{blaze::codec::NetAddress, env};

use actix_web::{
    get,
    http::header::ContentType,
    web::{Query, ServiceConfig},
    HttpResponse, Responder,
};
use log::debug;
use serde::Deserialize;

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(qos);
}

#[derive(Debug, Deserialize)]
pub struct QosQuery {
    #[serde(rename = "prpt")]
    port: u16,
}

#[get("/qos/qos")]
async fn qos(query: Query<QosQuery>) -> impl Responder {
    debug!("Recieved QOS query: (Port: {})", query.port);

    let ip = NetAddress::from_ipv4("127.0.0.1");
    let port: u16 = env::from_env(env::MAIN_PORT);

    let response = format!(
        r"<qos>
    <numprobes>0</numprobes>
    <qosport>{}</qosport>
    <probesize>0</probesize>
    <qosip>{}</qosip>
    <requestid>1</requestid>
    <reqsecret>0</reqsecret>
</qos>",
        port, ip.0
    );

    HttpResponse::Ok()
        .content_type(ContentType::xml())
        .body(response)
}
