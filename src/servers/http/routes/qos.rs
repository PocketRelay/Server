//! Routes for the Quality of Service server. Unknown whether any of the
//! response address and ports are correct however this request must succeed
//! or the client doesn't seem to know its external IP
use crate::{blaze::codec::NetAddress, env};
use axum::{
    extract::Query,
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use log::debug;
use serde::Deserialize;

/// Function for adding all the routes in this file to
/// the provided router
///
/// `router` The route to add to
pub fn route(router: Router) -> Router {
    router.route("/qos/qos", get(qos))
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
async fn qos(Query(query): Query<QosQuery>) -> Response {
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
    let mut res = response.into_response();
    res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(mime::TEXT_XML.as_ref()),
    );
    res
}