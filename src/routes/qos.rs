//! Routes for the Quality of Service server. Unknown whether any of the
//! response address and ports are correct however this request must succeed
//! or the client doesn't seem to know its external IP
use crate::middleware::xml::Xml;
use axum::extract::Query;
use indoc::formatdoc;
use log::debug;
use serde::Deserialize;

/// Query for the Qualitu Of Service route
#[derive(Deserialize)]
pub struct QosQuery {
    /// The port the client is using
    #[serde(rename = "prpt")]
    port: u16,
}

/// GET /qos/qos
///
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
pub async fn qos(Query(query): Query<QosQuery>) -> Xml {
    debug!("Recieved QOS query: (Port: {})", query.port);

    /// Port for the local Quality of Service server
    const QOS_PORT: u16 = 42130;
    const IP: u32 = u32::from_be_bytes([127, 0, 0, 1]);

    Xml(formatdoc! {r#"
        <?xml version="1.0" encoding="UTF-8"?>
        <qos> 
            <numprobes>0</numprobes>
            <qosport>{}</qosport>
            <probesize>0</probesize>
            <qoshost>127.0.0.1</qoshost>
            <qosip>{}</qosip>
            <requestid>1</requestid>
            <reqsecret>0</reqsecret>
        </qos>
    "#, QOS_PORT, IP
    })
}
