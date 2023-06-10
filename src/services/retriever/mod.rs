//! Retriever system for connecting and retrieving data from the official
//! Mass Effect 3 servers.

use std::fmt::{Debug, Display};

use self::origin::OriginFlowService;
use crate::{
    config::RetrieverConfig,
    utils::{
        components::{Components, Redirector},
        models::{InstanceDetails, InstanceNet, Port},
    },
};
use blaze_pk::{
    codec::{Decodable, Encodable},
    error::DecodeError,
    packet::{Packet, PacketCodec, PacketComponents, PacketDebug, PacketHeader, PacketType},
};
use blaze_ssl_async::{stream::BlazeStream, BlazeError};
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, log_enabled};
use models::InstanceRequest;
use reqwest;
use serde::Deserialize;
use thiserror::Error;
use tokio::io;
use tokio_util::codec::Framed;

mod models;
pub mod origin;

/// Structure for the retrievier system which contains the host address
/// for the official game server in order to make further connections
pub struct Retriever {
    instance: OfficialInstance,
    /// Optional service for creating origin flows if enabled
    pub origin_flow: Option<OriginFlowService>,
}

struct OfficialInstance {
    /// The host address of the official server
    host: String,
    /// The port of the official server.
    port: u16,
}

#[derive(Debug, Error)]
pub enum InstanceError {
    #[error("Failed to request lookup from google: {0}")]
    LookupRequest(#[from] reqwest::Error),
    #[error("Failed to lookup server response empty")]
    MissingValue,
    #[error("Failed to connect to server: {0}")]
    Blaze(#[from] BlazeError),
    #[error("Failed to retrieve instance: {0}")]
    InstanceRequest(#[from] RetrieverError),
}

impl OfficialInstance {
    /// The hostname for the redirector server
    const REDIRECTOR_HOST: &str = "gosredirector.ea.com";
    /// The port for the redirector server.
    const REDIRECT_PORT: Port = 42127;

    async fn obtain() -> Result<OfficialInstance, InstanceError> {
        let host = Self::lookup_host().await?;
        debug!("Completed host lookup: {}", &host);

        // Create a session to the redirector server
        let mut session = OfficialSession::connect(&host, Self::REDIRECT_PORT).await?;

        // Request the server instance
        let instance: InstanceDetails = session
            .request(
                Components::Redirector(Redirector::GetServerInstance),
                InstanceRequest,
            )
            .await?;

        // Extract the host and port turning the host into a string
        let InstanceNet { host, port } = instance.net;
        let host: String = host.into();

        debug!("Retriever setup complete. (Host: {} Port: {})", &host, port);
        Ok(OfficialInstance { host, port })
    }

    /// Attempts to resolve the address of the official gosredirector. First attempts
    /// to use the system DNS with tokio but if the resolved address is loopback it
    /// is ignored and the google HTTP DNS will be attempted instead
    ///
    /// `host` The host to lookup
    async fn lookup_host() -> Result<String, InstanceError> {
        let host = Self::REDIRECTOR_HOST;

        // Attempt to lookup using the system DNS
        {
            let tokio = tokio::net::lookup_host(host)
                .await
                .ok()
                .and_then(|mut value| value.next());

            if let Some(tokio) = tokio {
                let ip = tokio.ip();
                // Loopback value means it was probbably redirected in the hosts file
                // so those are ignored
                if !ip.is_loopback() {
                    return Ok(format!("{}", ip));
                }
            }
        }

        // Attempt to lookup using cloudflares DNS over HTTP

        let client = reqwest::Client::new();
        let url = format!("https://cloudflare-dns.com/dns-query?name={host}&type=A");
        let mut response: LookupResponse = client
            .get(url)
            .header("Accept", "application/dns-json")
            .send()
            .await?
            .json()
            .await?;

        response
            .answer
            .pop()
            .map(|value| value.data)
            .ok_or(InstanceError::MissingValue)
    }

    /// Creates a stream to the main server and wraps it with a
    /// session returning that session. Will return None if the
    /// stream failed.
    pub async fn session(&self) -> Option<OfficialSession> {
        OfficialSession::connect(&self.host, self.port).await.ok()
    }
}

impl Retriever {
    /// Attempts to create a new retriever by first retrieving the coorect
    /// ip address of the gosredirector.ea.com host and then creates a
    /// connection to the redirector server and obtains the IP and Port
    /// of the Official server.
    pub async fn new(config: RetrieverConfig) -> Option<Retriever> {
        if !config.enabled {
            return None;
        }

        let instance = match OfficialInstance::obtain().await {
            Ok(value) => value,
            Err(error) => {
                error!("Failed to setup redirector: {}", error);
                return None;
            }
        };

        let origin_flow = if config.origin_fetch {
            Some(OriginFlowService {
                data: config.origin_fetch_data,
            })
        } else {
            None
        };

        Some(Retriever {
            instance,
            origin_flow,
        })
    }

    /// Creates a stream to the main server and wraps it with a
    /// session returning that session. Will return None if the
    /// stream failed.
    pub async fn session(&self) -> Option<OfficialSession> {
        self.instance.session().await
    }
}

/// Session implementation for a retriever client
pub struct OfficialSession {
    /// The ID for the next request packet
    id: u16,
    /// The underlying SSL / TCP stream connection
    stream: Framed<BlazeStream, PacketCodec>,
}

/// Error type for retriever errors
#[derive(Debug, Error)]
pub enum RetrieverError {
    /// Packet decode errror
    #[error(transparent)]
    Decode(#[from] DecodeError),
    /// IO Error
    #[error(transparent)]
    IO(#[from] io::Error),
    /// Error response packet
    #[error(transparent)]
    Packet(#[from] ErrorPacket),
    /// Stream ended early
    #[error("Reached end of stream")]
    EarlyEof,
}

pub type RetrieverResult<T> = Result<T, RetrieverError>;

impl OfficialSession {
    async fn connect(host: &str, port: Port) -> Result<OfficialSession, BlazeError> {
        let stream = BlazeStream::connect((host, port)).await?;
        Ok(Self {
            id: 0,
            stream: Framed::new(stream, PacketCodec),
        })
    }

    /// Writes a request packet and waits until the response packet is
    /// recieved returning the contents of that response packet.
    pub async fn request<Req: Encodable, Res: Decodable>(
        &mut self,
        component: Components,
        contents: Req,
    ) -> RetrieverResult<Res> {
        let response = self.request_raw(component, contents).await?;
        let contents = response.decode::<Res>()?;
        Ok(contents)
    }

    /// Writes a request packet and waits until the response packet is
    /// recieved returning the contents of that response packet.
    pub async fn request_raw<Req: Encodable>(
        &mut self,
        component: Components,
        contents: Req,
    ) -> RetrieverResult<Packet> {
        let request = Packet::request(self.id, component, contents);

        debug_log_packet(&request, "Sending to Official");
        let header = request.header;

        self.stream.send(request).await?;

        self.id += 1;
        self.expect_response(&header).await
    }

    /// Writes a request packet and waits until the response packet is
    /// recieved returning the contents of that response packet. The
    /// request will have no content
    pub async fn request_empty<Res: Decodable>(
        &mut self,
        component: Components,
    ) -> RetrieverResult<Res> {
        let response = self.request_empty_raw(component).await?;
        let contents = response.decode::<Res>()?;
        Ok(contents)
    }

    /// Writes a request packet and waits until the response packet is
    /// recieved returning the raw response packet
    pub async fn request_empty_raw(&mut self, component: Components) -> RetrieverResult<Packet> {
        let request = Packet::request_empty(self.id, component);
        debug_log_packet(&request, "Sent to Official");
        let header = request.header;
        self.stream.send(request).await?;
        self.id += 1;
        self.expect_response(&header).await
    }

    /// Waits for a response packet to be recieved any notification packets
    /// that are recieved are handled in the handle_notify function.
    async fn expect_response(&mut self, request: &PacketHeader) -> RetrieverResult<Packet> {
        loop {
            let response = match self.stream.next().await {
                Some(value) => value?,
                None => return Err(RetrieverError::EarlyEof),
            };
            debug_log_packet(&response, "Received from Official");
            let header = &response.header;

            if let PacketType::Response = header.ty {
                if header.path_matches(request) {
                    return Ok(response);
                }
            } else if let PacketType::Error = header.ty {
                return Err(RetrieverError::Packet(ErrorPacket(response)));
            }
        }
    }
}

/// Logs the contents of the provided packet to the debug output along with
/// the header information.
///
/// `component` The component for the packet routing
/// `packet`    The packet that is being logged
/// `direction` The direction name for the packet
fn debug_log_packet(packet: &Packet, action: &str) {
    // Skip if debug logging is disabled
    if !log_enabled!(log::Level::Debug) {
        return;
    }
    let component = Components::from_header(&packet.header);
    let debug = PacketDebug {
        packet,
        component: component.as_ref(),
        minified: false,
    };
    debug!("\n{}\n{:?}", action, debug);
}

/// Wrapping structure for packets to allow them to be
/// used as errors
#[derive(Debug)]
pub struct ErrorPacket(Packet);

impl std::error::Error for ErrorPacket {}

impl Display for ErrorPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#X}", self.0.header.error)
    }
}

/// Structure for the lookup responses from the google DNS API
///
/// # Structure
///
/// ```
/// {
///   "Status": 0,
///   "TC": false,
///   "RD": true,
///   "RA": true,
///   "AD": false,
///   "CD": false,
///   "Question": [
///     {
///       "name": "gosredirector.ea.com.",
///       "type": 1
///     }
///   ],
///   "Answer": [
///     {
///       "name": "gosredirector.ea.com.",
///       "type": 1,
///       "TTL": 300,
///       "data": "159.153.64.175"
///     }
///   ],
///   "Comment": "Response from 2600:1403:a::43."
/// }
/// ```
#[derive(Deserialize)]
struct LookupResponse {
    #[serde(rename = "Answer")]
    answer: Vec<Answer>,
}

/// Structure for answer portion of request. Only the data value is
/// being used so only that is present here.
///
/// # Structure
/// ```
/// {
///   "name": "gosredirector.ea.com.",
///   "type": 1,
///   "TTL": 300,
///   "data": "159.153.64.175"
/// }
/// ```
#[derive(Deserialize)]
struct Answer {
    data: String,
}
