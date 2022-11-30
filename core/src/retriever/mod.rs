//! Module for retrieving data from the official Mass Effect 3 Servers

use blaze_pk::{
    codec::{Decodable, Encodable},
    packet::{Packet, PacketType},
};
use blaze_ssl_async::stream::{BlazeStream, StreamMode};
use log::{debug, error};
use tokio::net::TcpStream;

use crate::{
    blaze::{
        codec::{InstanceDetails, Port},
        components::{Components, Redirector},
        errors::BlazeResult,
    },
    env,
    retriever::codec::InstanceRequest,
};

use utils::net::lookup_host;

mod codec;
pub mod origin;

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
    const REDIRECT_PORT: Port = 42127;

    /// Attempts to create a new retriever by first retrieving the coorect
    /// ip address of the gosredirector.ea.com host and then creates a
    /// connection to the redirector server and obtains the IP and Port
    /// of the Official server.
    pub async fn new() -> Option<Retriever> {
        if !env::from_env(env::RETRIEVER) {
            return None;
        }

        let redirector_host = lookup_host(Self::REDIRECTOR_HOST).await?;
        let (host, port) = Self::get_main_host(redirector_host).await?;
        debug!("Retriever setup complete. (Host: {} Port: {})", &host, port);
        Some(Retriever { host, port })
    }

    /// Makes a instance request to the redirect server at the provided
    /// host and returns the instance response.
    async fn get_main_host(host: String) -> Option<(String, Port)> {
        debug!("Connecting to official redirector");
        let stream = Self::stream_to(&host, Self::REDIRECT_PORT).await?;
        let mut session = RetSession::new(stream)?;
        debug!("Connected to official redirector");
        debug!("Requesting details from official server");
        let instance = session.get_main_instance().await.ok()?;
        let net = instance.net;
        Some((net.host.into(), net.port))
    }

    /// Returns a new session to the main server
    pub async fn session(&self) -> Option<RetSession> {
        let stream = self.stream().await?;
        RetSession::new(stream)
    }

    /// Returns a new stream to the mian server
    pub async fn stream_to(host: &String, port: Port) -> Option<Stream> {
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
        BlazeStream::new(stream, StreamMode::Client)
            .await
            .map_err(|err| {
                error!(
                    "Failed to connect to server at {}:{}; Cause: {err:?}",
                    host, port
                );
                err
            })
            .ok()
    }

    /// Returns a new stream to the main server
    pub async fn stream(&self) -> Option<Stream> {
        Self::stream_to(&self.host, self.port).await
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
    pub fn new(stream: Stream) -> Option<Self> {
        Some(Self { id: 0, stream })
    }

    /// Handler for notification type packets that are encountered
    /// while expecting a response packet from the server.
    pub async fn handle_notify(
        &mut self,
        component: Components,
        _value: &Packet,
    ) -> BlazeResult<()> {
        debug!("Got notify packet: {component:?}");
        Ok(())
    }

    /// Writes a request packet and waits until the response packet is
    /// recieved returning the contents of that response packet.
    pub async fn request<Req: Encodable, Res: Decodable>(
        &mut self,
        component: Components,
        contents: Req,
    ) -> BlazeResult<Res> {
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
    ) -> BlazeResult<Packet> {
        let request = Packet::request(self.id, component, contents);
        request.write_blaze(&mut self.stream)?;
        self.stream.flush().await?;
        self.id += 1;
        self.expect_response(&request).await
    }

    /// Writes a request packet and waits until the response packet is
    /// recieved returning the contents of that response packet. The
    /// request will have no content
    pub async fn request_empty<Res: Decodable>(
        &mut self,
        component: Components,
    ) -> BlazeResult<Res> {
        let response = self.request_empty_raw(component).await?;
        let contents = response.decode::<Res>()?;
        Ok(contents)
    }

    /// Writes a request packet and waits until the response packet is
    /// recieved returning the raw response packet
    pub async fn request_empty_raw(&mut self, component: Components) -> BlazeResult<Packet> {
        let request = Packet::request_empty(self.id, component);
        request.write_blaze(&mut self.stream)?;
        self.stream.flush().await?;
        self.id += 1;
        self.expect_response(&request).await
    }

    /// Waits for a response packet to be recieved any notification packets
    /// that are recieved are handled in the handle_notify function.
    async fn expect_response(&mut self, request: &Packet) -> BlazeResult<Packet> {
        loop {
            let (component, response) = Packet::read_blaze_typed(&mut self.stream).await?;
            let header = &request.header;
            if header.ty == PacketType::Notify {
                self.handle_notify(component, &response).await.ok();
            } else if header.path_matches(&request.header) {
                return Ok(response);
            }
        }
    }

    /// Function for making the request for the official server instance
    /// from the redirector server.
    async fn get_main_instance(&mut self) -> BlazeResult<InstanceDetails> {
        self.request::<InstanceRequest, InstanceDetails>(
            Components::Redirector(Redirector::GetServerInstance),
            InstanceRequest,
        )
        .await
    }
}
