//! Module for retrieving data from the official Mass Effect 3 Servers

use std::{io::Write, net::TcpStream, sync::atomic::AtomicU16};

use blaze_pk::{Codec, OpaquePacket, Packets, SimpleCounter};
use blaze_ssl::stream::{BlazeResult, BlazeStream, StreamMode};
use dnsclient::{sync::DNSClient, UpstreamServer};
use log::{debug, error};
use tokio::task::spawn_blocking;

use crate::blaze::components::Components;

mod shared;

/// Structure for the retrievier system which contains the host address
/// for the official game server in order to make further connections
pub struct Retriever {
    host: String,
    port: u16,
}

impl Retriever {
    /// The address of the DNS server to use for DNS lookups
    const DNS_ADDRESS: (&str, u16) = ("1.1.1.1", 53);
    const REDIRECT_PORT: u16 = 42127;

    pub async fn new() -> Option<Retriever> {
        let redirector_host = spawn_blocking(Self::get_redirector_host).await.ok()??;
    }

    /// Attempts to find the real address for gosredirector.ea.com
    /// will use the cloudflare DNS in order to bypass any possible
    /// redirects present in the system hosts file.
    ///
    /// Will return None if this process failed.
    fn get_redirector_host() -> Option<String> {
        debug!("Attempting lookup for gosredirector.ea.com")
        let upstream = UpstreamServer::new(DNS_ADDRESS);
        let dns_client = DNSClient::new(vec![upstream]);
        let result = dns_client.query_a(name).ok()?;
        let result = result.pop()?;
        let ip = format!("{}", result);
        debug!("Lookup Complete: {}", &ip);
        Some(ip)
    }

    fn get_main_host(host: &str) -> Option<String> {
        debug!("Connecting to official redirector");
        let stream = TcpStream::connect((host, Self::REDIRECT_PORT))
            .map_err(|err| {
                error!(
                    "Failed to connect to redirector server at {}:{}; Cause: {err:?}",
                    host,
                    Self::REDIRECT_PORT
                );
                err
            })
            .ok()?;
        let stream = BlazeStream::new(stream, StreamMode::Client)
            .map_err(|err| {
                error!(
                    "Failed to connect to redirector server at {}:{}; Cause: {err:?}",
                    host,
                    Self::REDIRECT_PORT
                );
                err
            })
            .ok()?;
        let mut session = RetSession::new(stream);
        debug!("Connected to official redirector");


    }
}

/// Session implementation for a retriever client
struct RetSession {
    counter: SimpleCounter,
    stream: BlazeStream<TcpStream>,
}

impl RetSession {
    pub fn new(stream: BlazeStream<TcpStream>) -> Self {
        Self {
            counter: SimpleCounter::new(),
            stream,
        }
    }

    pub fn request<T: Codec>(&mut self, component: Components, contents: &T) -> BlazeResult<()> {
        let packet = Packets::request(&mut self.counter, component, contents);
        packet.write(&mut self.stream)?;
        Ok(())
    }
}
