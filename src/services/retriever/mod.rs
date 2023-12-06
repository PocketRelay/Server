//! Retriever system for connecting and retrieving data from the official
//! Mass Effect 3 servers.

use self::origin::OriginFlowService;
use crate::{
    config::RetrieverConfig,
    session::{
        models::{InstanceDetails, InstanceNet, Port},
        packet::{FireFrame, FrameType, Packet, PacketCodec, PacketDebug},
    },
    utils::components::redirector,
};
use blaze_ssl_async::{stream::BlazeStream, BlazeError};
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, log_enabled};
use models::InstanceRequest;
use origin::OriginFlow;
use reqwest;
use serde::Deserialize;
use std::{
    fmt::Display,
    ops::Add,
    time::{Duration, SystemTime},
};
use tdf::{DecodeError, TdfDeserialize, TdfSerialize};
use thiserror::Error;
use tokio::{io, sync::RwLock};
use tokio_util::codec::Framed;

mod models;
pub mod origin;

/// Structure for the retrievier system which contains the host address
/// for the official game server in order to make further connections
pub struct Retriever {
    // Optional official instance if fetching is possible
    instance: RwLock<Option<OfficialInstance>>,

    /// Optional service for creating origin flows if enabled
    origin_flow: Option<OriginFlowService>,
}

#[derive(Debug, Error)]
pub enum GetFlowError {
    #[error("Retriever is disabled or unavailable")]
    Unavailable,
    #[error("Unable to obtain retriever instance")]
    Instance,
    #[error("Failed to obtain session")]
    Session,
    #[error("Origin authentication is not enabled")]
    OriginDisabled,
}

/// Connection details for an official server instance
struct OfficialInstance {
    /// The host address of the official server
    host: String,
    /// The port of the official server.
    port: u16,
    /// The time the instance should expire at
    expiry: SystemTime,
}

/// Errors that could occur while attempting to obtain
/// an official server instance details
#[derive(Debug, Error)]
pub enum InstanceError {
    #[error("Failed to request lookup from cloudflare: {0}")]
    LookupRequest(#[from] reqwest::Error),
    #[error("Failed to lookup server response empty")]
    MissingValue,
    #[error("Failed to connect to server: {0}")]
    Blaze(#[from] BlazeError),
    #[error("Failed to retrieve instance: {0}")]
    InstanceRequest(#[from] RetrieverError),
    #[error("Server response missing address")]
    MissingAddress,
}

impl OfficialInstance {
    /// Time an official instance should be considered valid for (2 hours)
    const LIFETIME: Duration = Duration::from_secs(60 * 60 * 2);

    /// The hostname for the redirector server
    ///
    /// If this service goes down the same logic is available
    /// from https://winter15.gosredirector.ea.com:42230/redirector/getServerInstance
    /// using an XML structure:
    ///
    /// <?xml version="1.0" encoding="UTF-8"?>
    ///    <serverinstancerequest>
    ///    <blazesdkversion>3.15.6.0</blazesdkversion>
    ///    <blazesdkbuilddate>Dec 21 2012 12:47:10</blazesdkbuilddate>
    ///    <clientname>MassEffect3-pc</clientname>
    ///    <clienttype>CLIENT_TYPE_GAMEPLAY_USER</clienttype>
    ///    <clientplatform>pc</clientplatform>
    ///    <clientskuid>pc</clientskuid>
    ///    <clientversion>05427.124</clientversion>
    ///    <dirtysdkversion>8.14.7.1</dirtysdkversion>
    ///    <environment>prod</environment>
    ///    <clientlocale>1701729619</clientlocale>
    ///    <name>masseffect-3-pc</name>
    ///    <platform>Windows</platform>
    ///    <connectionprofile>standardSecure_v3</connectionprofile>
    ///    <istrial>0</istrial>
    /// </serverinstancerequest>
    const REDIRECTOR_HOST: &'static str = "gosredirector.ea.com";
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
                redirector::COMPONENT,
                redirector::GET_SERVER_INSTANCE,
                InstanceRequest,
            )
            .await?;

        // Extract the host and port turning the host into a string
        let (host, port) = match instance.net {
            InstanceNet::InstanceAddress(addr) => (addr.host, addr.port),
            _ => return Err(InstanceError::MissingAddress),
        };
        let host: String = host.into();

        debug!(
            "Retriever instance obtained. (Host: {} Port: {})",
            &host, port
        );

        let expiry = SystemTime::now().add(Self::LIFETIME);

        Ok(OfficialInstance { host, port, expiry })
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
    pub async fn start(config: RetrieverConfig) -> Retriever {
        let instance = if config.enabled {
            match OfficialInstance::obtain().await {
                Ok(value) => Some(value),
                Err(error) => {
                    error!("Failed to setup redirector: {}", error);
                    None
                }
            }
        } else {
            None
        };

        let origin_flow = if config.origin_fetch {
            Some(OriginFlowService {
                data: config.origin_fetch_data,
            })
        } else {
            None
        };

        Retriever {
            instance: RwLock::new(instance),
            origin_flow,
        }
    }

    pub async fn origin_flow(&self) -> Result<OriginFlow, GetFlowError> {
        let flow = self
            .origin_flow
            .as_ref()
            .ok_or(GetFlowError::OriginDisabled)?;

        let read_guard = self.instance.read().await;
        let instance = read_guard.as_ref().ok_or(GetFlowError::Unavailable)?;
        let is_expired = instance.expiry < SystemTime::now();

        let guard = if is_expired {
            // Drop the read instance and guard
            let _ = instance;
            drop(read_guard);

            debug!("Current official instance is outdated.. retrieving a new instance");
            let mut write_guard = self.instance.write().await;

            let official = match OfficialInstance::obtain().await {
                Ok(value) => Some(value),
                Err(err) => {
                    error!(
                        "Official server instance expired but failed to obtain new instance: {}",
                        err
                    );
                    None
                }
            };

            *write_guard = official;

            write_guard.downgrade()
        } else {
            read_guard
        };

        let instance = guard.as_ref().ok_or(GetFlowError::Instance)?;
        let session = instance.session().await.ok_or(GetFlowError::Session)?;

        Ok(flow.create(session))
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
    /// Creates a session with an official server at the provided
    /// `host` and `port`
    async fn connect(host: &str, port: Port) -> Result<OfficialSession, BlazeError> {
        let stream = BlazeStream::connect((host, port)).await?;
        Ok(Self {
            id: 0,
            stream: Framed::new(stream, PacketCodec::default()),
        })
    }

    /// Writes a request packet and waits until the response packet is
    /// recieved returning the contents of that response packet.
    pub async fn request<Req, Res>(
        &mut self,
        component: u16,
        command: u16,
        contents: Req,
    ) -> RetrieverResult<Res>
    where
        Req: TdfSerialize,
        for<'a> Res: TdfDeserialize<'a> + 'a,
    {
        let response = self.request_raw(component, command, contents).await?;
        let contents = response.deserialize::<Res>()?;
        Ok(contents)
    }

    /// Writes a request packet and waits until the response packet is
    /// recieved returning the contents of that response packet.
    pub async fn request_raw<Req: TdfSerialize>(
        &mut self,
        component: u16,
        command: u16,
        contents: Req,
    ) -> RetrieverResult<Packet> {
        let request = Packet::request(self.id, component, command, contents);

        debug_log_packet(&request, "Send");
        let frame = request.frame.clone();

        self.stream.send(request).await?;

        self.id += 1;
        self.expect_response(&frame).await
    }

    /// Writes a request packet and waits until the response packet is
    /// recieved returning the contents of that response packet. The
    /// request will have no content
    pub async fn request_empty<Res>(&mut self, component: u16, command: u16) -> RetrieverResult<Res>
    where
        for<'a> Res: TdfDeserialize<'a> + 'a,
    {
        let response = self.request_empty_raw(component, command).await?;
        let contents = response.deserialize::<Res>()?;
        Ok(contents)
    }

    /// Writes a request packet and waits until the response packet is
    /// recieved returning the raw response packet
    pub async fn request_empty_raw(
        &mut self,
        component: u16,
        command: u16,
    ) -> RetrieverResult<Packet> {
        let request = Packet::request_empty(self.id, component, command);
        debug_log_packet(&request, "Send");
        let header = request.frame.clone();
        self.stream.send(request).await?;
        self.id += 1;
        self.expect_response(&header).await
    }

    /// Waits for a response packet to be recieved any notification packets
    /// that are recieved are handled in the handle_notify function.
    async fn expect_response(&mut self, request: &FireFrame) -> RetrieverResult<Packet> {
        loop {
            let response = match self.stream.next().await {
                Some(value) => value?,
                None => return Err(RetrieverError::EarlyEof),
            };
            debug_log_packet(&response, "Receive");
            let header = &response.frame;

            match &header.ty {
                FrameType::Response => {
                    if header.path_matches(request) {
                        return Ok(response);
                    }
                }
                FrameType::Error => return Err(RetrieverError::Packet(ErrorPacket(response))),
                _ => {}
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
    let debug = PacketDebug { packet };
    debug!("\nOfficial: {}\n{:?}", action, debug);
}

/// Wrapping structure for packets to allow them to be
/// used as errors
#[derive(Debug)]
pub struct ErrorPacket(Packet);

impl std::error::Error for ErrorPacket {}

impl Display for ErrorPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#X}", self.0.frame.error)
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
