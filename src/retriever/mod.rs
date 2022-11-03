//! Module for retrieving data from the official Mass Effect 3 Servers

use std::{
    io::Write,
    net::{Ipv4Addr, SocketAddrV4, TcpStream},
};

use blaze_pk::{Codec, OpaquePacket, PacketType, Packets};
use blaze_ssl::stream::{BlazeStream, StreamMode};
use dnsclient::{sync::DNSClient, UpstreamServer};
use log::{debug, error};
use tokio::task::spawn_blocking;

use crate::{
    blaze::{
        components::{Components, Redirector},
        errors::{BlazeError, BlazeResult},
    },
    retriever::shared::{InstanceRequest, InstanceResponse},
};

mod shared;

/// Type for SSL wrapped blaze stream
type Stream = BlazeStream<TcpStream>;

/// Structure for the retrievier system which contains the host address
/// for the official game server in order to make further connections
pub struct Retriever {
    host: String,
    port: u16,
    secu: bool,
}

impl Retriever {
    const REDIRECT_PORT: u16 = 42127;

    pub async fn new() -> Option<Retriever> {
        let redirector_host = spawn_blocking(Self::get_redirector_host).await.ok()??;
        let main_host = spawn_blocking(move || Self::get_main_host(redirector_host))
            .await
            .ok()??;
        debug!("Retriever setup complete.");
        Some(Retriever {
            host: main_host.host,
            port: main_host.port,
            secu: main_host.secu,
        })
    }

    /// Attempts to find the real address for gosredirector.ea.com
    /// will use the cloudflare DNS in order to bypass any possible
    /// redirects present in the system hosts file.
    ///
    /// Will return None if this process failed.
    fn get_redirector_host() -> Option<String> {
        debug!("Attempting lookup for gosredirector.ea.com");
        let addr = SocketAddrV4::new(Ipv4Addr::new(1, 1, 1, 1), 53);
        let upstream = UpstreamServer::new(addr);
        let dns_client = DNSClient::new(vec![upstream]);
        let mut result = dns_client.query_a("gosredirector.ea.com").ok()?;
        let result = result.pop()?;
        let ip = format!("{}", result);
        debug!("Lookup Complete: {}", &ip);
        Some(ip)
    }

    fn session(host: &str, port: u16) -> Option<RetSession> {
        let addr = (host.clone(), port);
        let stream = TcpStream::connect(addr)
            .map_err(|err| {
                error!(
                    "Failed to connect to redirector server at {}:{}; Cause: {err:?}",
                    host, port
                );
                err
            })
            .ok()?;
        let stream = BlazeStream::new(stream, StreamMode::Client)
            .map_err(|err| {
                error!(
                    "Failed to connect to redirector server at {}:{}; Cause: {err:?}",
                    host, port
                );
                err
            })
            .ok()?;
        Some(RetSession::new(stream))
    }

    fn get_main_host(host: String) -> Option<InstanceResponse> {
        debug!("Connecting to official redirector");
        let mut session = Self::session(&host, Self::REDIRECT_PORT)?;
        debug!("Connected to official redirector");
        debug!("Requesting details from official server");
        session.get_main_instance().ok()
    }
}

/// Session implementation for a retriever client
struct RetSession {
    id: u16,
    stream: Stream,
}

impl RetSession {
    pub fn new(stream: Stream) -> Self {
        Self { id: 0, stream }
    }

    pub fn handle_notify(
        &mut self,
        component: Components,
        value: &OpaquePacket,
    ) -> BlazeResult<()> {
        debug!("Got notify packet: {component:?}");
        value.debug_decode()?;
        Ok(())
    }

    /// Writes a request packet returning the recieved response packet
    pub fn request<Req: Codec, Res: Codec>(
        &mut self,
        component: Components,
        contents: &Req,
    ) -> BlazeResult<Res> {
        let request = Packets::request(self.id, component, contents);
        request.write(&mut self.stream)?;
        self.stream.flush()?;
        self.id += 1;
        self.expect_response(&request)
    }

    fn expect_response<T: Codec>(&mut self, request: &OpaquePacket) -> BlazeResult<T> {
        loop {
            let (component, response): (Components, OpaquePacket) =
                match OpaquePacket::read_typed(&mut self.stream) {
                    Ok(value) => value,
                    Err(_) => return Err(BlazeError::Other("Unable to read / decode packet")),
                };
            if response.0.ty == PacketType::Notify {
                self.handle_notify(component, &response).ok();
                continue;
            }
            if !response.0.path_matches(&request.0) {
                continue;
            }
            let contents = response.contents::<T>()?;
            return Ok(contents);
        }
    }

    fn get_main_instance(&mut self) -> BlazeResult<InstanceResponse> {
        self.request::<InstanceRequest, InstanceResponse>(
            Components::Redirector(Redirector::GetServerInstance),
            &InstanceRequest,
        )
        .map_err(|err| {
            error!("Failed to request server instance: {err:?}");
            BlazeError::Other("Unable to obtain main instance")
        })
    }
}
