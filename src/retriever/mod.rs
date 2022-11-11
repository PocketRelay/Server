//! Module for retrieving data from the official Mass Effect 3 Servers

use blaze_pk::{Codec, OpaquePacket, PacketType, Packets};
use blaze_ssl_async::stream::{BlazeStream, StreamMode};
use log::{debug, error};
use tokio::net::TcpStream;

use crate::{
    blaze::{
        components::{Components, Redirector},
        errors::{BlazeError, BlazeResult},
    },
    env,
    retriever::shared::{InstanceRequest, InstanceResponse},
};

use utils::dns::lookup_host;

pub mod origin;
mod shared;

#[cfg(test)]
mod test;

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
        if !env::bool_env(env::RETRIEVER) {
            return None;
        }

        let redirector_host = lookup_host(Self::REDIRECTOR_HOST).await?;
        let (host, port) = Self::get_main_host(redirector_host).await?;
        debug!("Retriever setup complete. (Host: {} Port: {})", &host, port);
        Some(Retriever { host, port })
    }

    /// Makes a instance request to the redirect server at the provided
    /// host and returns the instance response.
    async fn get_main_host(host: String) -> Option<(String, u16)> {
        debug!("Connecting to official redirector");
        let mut session = RetSession::new(&host, Self::REDIRECT_PORT).await?;
        debug!("Connected to official redirector");
        debug!("Requesting details from official server");
        let instance = session.get_main_instance().await.ok()?;
        Some((instance.host, instance.port))
    }

    /// Returns a new session to the main server
    pub async fn session(&self) -> Option<RetSession> {
        RetSession::new(&self.host, self.port).await
    }
}

/// Session implementation for a retriever client
pub struct RetSession {
    /// The ID for the next request packet
    id: u16,
    /// The underlying SSL / TCP stream connection
    stream: Stream,
}

impl RetSession {
    /// Creates a new retriever session for the provided host and
    /// port. This will create the underlying connection aswell.
    /// If creating the connection fails then None is returned instead.
    pub async fn new(host: &str, port: u16) -> Option<Self> {
        let addr = (host.clone(), port);
        let stream = TcpStream::connect(addr)
            .await
            .map_err(|err| {
                error!(
                    "Failed to connect to server at {}:{}; Cause: {err:?}",
                    host, port
                );
                err
            })
            .ok()?;
        let stream = BlazeStream::new(stream, StreamMode::Client)
            .await
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
    pub async fn handle_notify(
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
    pub async fn request<Req: Codec, Res: Codec>(
        &mut self,
        component: Components,
        contents: &Req,
    ) -> BlazeResult<Res> {
        let request = Packets::request(self.id, component, contents);
        request.write_blaze(&mut self.stream)?;
        self.stream.flush().await?;
        self.id += 1;
        let response = self.expect_response(&request).await?;
        let contents = response.contents::<Res>()?;
        Ok(contents)
    }

    /// Writes a request packet and waits until the response packet is
    /// recieved returning the contents of that response packet.
    pub async fn request_raw<Req: Codec>(
        &mut self,
        component: Components,
        contents: &Req,
    ) -> BlazeResult<OpaquePacket> {
        let request = Packets::request(self.id, component, contents);
        request.write_blaze(&mut self.stream)?;
        self.stream.flush().await?;
        self.id += 1;
        self.expect_response(&request).await
    }

    /// Writes a request packet and waits until the response packet is
    /// recieved returning the contents of that response packet. The
    /// request will have no content
    pub async fn request_empty<Res: Codec>(&mut self, component: Components) -> BlazeResult<Res> {
        let request = Packets::request_empty(self.id, component);
        request.write_blaze(&mut self.stream)?;
        self.stream.flush().await?;
        self.id += 1;
        let response = self.expect_response(&request).await?;
        let contents = response.contents::<Res>()?;
        Ok(contents)
    }

    /// Writes a request packet and waits until the response packet is
    /// recieved returning the raw response packet
    pub async fn request_empty_raw(&mut self, component: Components) -> BlazeResult<OpaquePacket> {
        let request = Packets::request_empty(self.id, component);
        request.write_blaze(&mut self.stream)?;
        self.stream.flush().await?;
        self.id += 1;
        self.expect_response(&request).await
    }

    /// Waits for a response packet to be recieved any notification packets
    /// that are recieved are handled in the handle_notify function.
    async fn expect_response(&mut self, request: &OpaquePacket) -> BlazeResult<OpaquePacket> {
        loop {
            let (component, response): (Components, OpaquePacket) =
                match OpaquePacket::read_async_typed_blaze(&mut self.stream).await {
                    Ok(value) => value,
                    Err(_) => return Err(BlazeError::Other("Unable to read / decode packet")),
                };
            if response.0.ty == PacketType::Notify {
                self.handle_notify(component, &response).await.ok();
                continue;
            }
            if !response.0.path_matches(&request.0) {
                continue;
            }
            return Ok(response);
        }
    }

    /// Function for making the request for the official server instance
    /// from the redirector server.
    async fn get_main_instance(&mut self) -> BlazeResult<InstanceResponse> {
        self.request::<InstanceRequest, InstanceResponse>(
            Components::Redirector(Redirector::GetServerInstance),
            &InstanceRequest,
        )
        .await
        .map_err(|err| {
            error!("Failed to request server instance: {err:?}");
            BlazeError::Other("Unable to obtain main instance")
        })
    }
}
