use core::env;

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

    let ext_host = env::str_env(env::EXT_HOST);
    let port: u16 = 17499;

    let response = format!(
        r"<qos>
    <numprobes>0</numprobes>
    <qosport>{}</qosport>
    <probesize>0</probesize>
    <qosip>{}</qosip>
    <requestid>1</requestid>
    <reqsecret>0</reqsecret>
</qos>",
        port, ext_host
    );

    HttpResponse::Ok()
        .content_type(ContentType::xml())
        .body(response)
}
