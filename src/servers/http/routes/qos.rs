//! Routes for the Quality of Service server. Unknown whether any of the
//! response address and ports are correct however this request must succeed
//! or the client doesn't seem to know its external IP
use crate::{
    servers::http::ext::Xml,
    utils::{env, models::NetAddress, net::public_address},
};
use axum::{extract::Query, routing::get, Router};
use log::debug;
use serde::Deserialize;

/// Router function creates a new router with all the underlying
/// routes for this file.
///
/// Prefix: /qos
pub fn router() -> Router {
    Router::new().route("/qos", get(qos))
}

/// Query for the Qualitu Of Service route
#[derive(Deserialize)]
struct QosQuery {
    /// The port the client is using
    #[serde(rename = "prpt")]
    port: u16,
}

/// Route accessed by the client for Quality Of Service connection. The IP and
/// port here are just replaced with that of the Main server.
///
/// Called by game: /qos/qos?prpt=3659&vers=1&qtyp=1
///
/// ```
/// <qos>
///     <numprobes>0</numprobes>
///     <qosport>17499</qosport>
///     <probesize>0</probesize>
///     <qosip>2733913518/* 162.244.53.174 */</qosip>
///     <requestid>1</requestid>
///     <reqsecret>0</reqsecret>
/// </qos>
///```
///
/// `query` The query string from the client
async fn qos(Query(query): Query<QosQuery>) -> Xml {
    debug!("Recieved QOS query: (Port: {})", query.port);

    let public_ip = public_address()
        .await
        .map(|value| NetAddress::from_ipv4(&value))
        .unwrap_or_default();

    // let ip: u32 = 2130706433 /* NetAddress::from_ipv4("127.0.0.1") */;
    let port: u16 = env::from_env(env::TELEMETRY_PORT);

    let response = format!(
        r"<qos> <numprobes>0</numprobes>
    <qosport>{}</qosport>
    <probesize>0</probesize>
    <qosip>{}</qosip>
    <requestid>1</requestid>
    <reqsecret>0</reqsecret>
</qos>",
        port, public_ip.0
    );
    Xml(response)
}
