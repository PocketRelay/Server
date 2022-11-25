//! Routes for the Quality of Service server. Unknown whether any of the
//! response address and ports are correct however this request must succeed
//! or the client doesn't seem to know its external IP
use actix_web::{
    get,
    http::header::ContentType,
    web::{Query, ServiceConfig},
    HttpResponse, Responder,
};
use core::{blaze::codec::NetAddress, env};
use log::debug;
use serde::Deserialize;

/// Function for configuring the services in this route
///
/// `cfg` Service config to configure
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(qos);
}

/// Query for the Qualitu Of Service route
#[derive(Debug, Deserialize)]
pub struct QosQuery {
    /// The port the client is using
    #[serde(rename = "prpt")]
    port: u16,
}

/// Route accessed by the client for Quality Of Service connection. The IP and
/// port here are just replaced with that of the Main server.
///
/// `query` The query string from the client
#[get("/qos/qos")]
async fn qos(query: Query<QosQuery>) -> impl Responder {
    debug!("Recieved QOS query: (Port: {})", query.port);

    let ip = NetAddress::from_ipv4("127.0.0.1");
    let port: u16 = env::from_env(env::MAIN_PORT);

    let response = format!(
        r"<qos> <numprobes>0</numprobes>
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
