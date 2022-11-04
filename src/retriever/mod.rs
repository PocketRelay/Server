//! Module for retrieving data from the official Mass Effect 3 Servers

use std::{io::Write, net::TcpStream};

use blaze_pk::{Codec, OpaquePacket, PacketType, Packets};
use blaze_ssl::stream::{BlazeStream, StreamMode};
use log::{debug, error};
use tokio::task::spawn_blocking;

use crate::{
    blaze::{
        components::{Components, Redirector},
        errors::{BlazeError, BlazeResult},
    },
    retriever::shared::{InstanceRequest, InstanceResponse},
    utils::dns::lookup_host,
};

mod shared;

/// Type for SSL wrapped blaze stream
type Stream = BlazeStream<TcpStream>;

/// Structure for the retrievier system which contains the host address
/// for the official game server in order to make further connections
pub struct Retriever {
    /// The host address of the official server
    host: String,
    /// The port of the official server.
    port: u16,
}

impl Retriever {
    /// The hostname for the redirector server
    const REDIRECTOR_HOST: &str = "gosredirector.ea.com";
    /// The port for the redirector server.
    const REDIRECT_PORT: u16 = 42127;

    /// Attempts to create a new retriever by first retrieving the coorect
    /// ip address of the gosredirector.ea.com host and then creates a
    /// connection to the redirector server and obtains the IP and Port
    /// of the Official server.
    pub async fn new() -> Option<Retriever> {
        let redirector_host = lookup_host(Self::REDIRECTOR_HOST).await?;
        let (host, port) = spawn_blocking(move || Self::get_main_host(redirector_host))
            .await
            .ok()??;
        debug!("Retriever setup complete. (Host: {} Port: {})", &host, port);
        Some(Retriever { host, port })
    }

    /// Makes a instance request to the redirect server at the provided
    /// host and returns the instance response.
    fn get_main_host(host: String) -> Option<(String, u16)> {
        debug!("Connecting to official redirector");
        let mut session = RetSession::new(&host, Self::REDIRECT_PORT)?;
        debug!("Connected to official redirector");
        debug!("Requesting details from official server");
        let instance = session.get_main_instance().ok()?;
        Some((instance.host, instance.port))
    }
}

/// Session implementation for a retriever client
struct RetSession {
    /// The ID for the next request packet
    id: u16,
    /// The underlying SSL / TCP stream connection
    stream: Stream,
}

impl RetSession {
    /// Creates a new retriever session for the provided host and
    /// port. This will create the underlying connection aswell.
    /// If creating the connection fails then None is returned instead.
    pub fn new(host: &str, port: u16) -> Option<Self> {
        let addr = (host.clone(), port);
        let stream = TcpStream::connect(addr)
            .map_err(|err| {
                error!(
                    "Failed to connect to server at {}:{}; Cause: {err:?}",
                    host, port
                );
                err
            })
            .ok()?;
        let stream = BlazeStream::new(stream, StreamMode::Client)
            .map_err(|err| {
                error!(
                    "Failed to connect to server at {}:{}; Cause: {err:?}",
                    host, port
                );
                err
            })
            .ok()?;
        Some(Self { id: 0, stream })
    }

    /// Handler for notification type packets that are encountered
    /// while expecting a response packet from the server.
    pub fn handle_notify(
        &mut self,
        component: Components,
        value: &OpaquePacket,
    ) -> BlazeResult<()> {
        debug!("Got notify packet: {component:?}");
        value.debug_decode()?;
        Ok(())
    }

    /// Writes a request packet and waits until the response packet is
    /// recieved returning the contents of that response packet.
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

    /// Waits for a response packet to be recieved any notification packets
    /// that are recieved are handled in the handle_notify function.
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

    /// Function for making the request for the official server instance
    /// from the redirector server.
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
