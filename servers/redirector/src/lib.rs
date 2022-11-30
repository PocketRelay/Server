//! Module for the Redirector server which handles redirecting the clients
//! to the correct address for the main server.

use core::blaze::codec::{InstanceDetails, InstanceNet};
use core::blaze::components::{Components, Redirector};
use core::blaze::errors::{BlazeError, BlazeResult};
use core::constants;
use core::{env, state::GlobalState};
use std::net::SocketAddr;
use std::time::Duration;

use blaze_pk::packet::Packet;

use blaze_ssl_async::stream::{BlazeStream, StreamMode};
use log::{debug, error};
use tokio::net::TcpStream;
use tokio::select;
use tokio::time::sleep;
use utils::net::{accept_stream, listener};

/// Starts the Redirector server this server is what the Mass Effect 3 game
/// client initially reaches out to. This server is responsible for telling
/// the client where the server is and whether it should use SSLv3 to connect.
pub async fn start_server() {
    let listener = listener("Redirector", env::from_env(env::REDIRECTOR_PORT)).await;
    let mut shutdown = GlobalState::shutdown();
    while let Some((stream, addr)) = accept_stream(&listener, &mut shutdown).await {
        tokio::spawn(async move {
            if let Err(err) = handle_client(stream, addr).await {
                error!("Unable to handle redirect: {err}");
            };
        });
    }
}

/// The timeout before idle redirector connections are terminated
/// (1 minutes before disconnect timeout)
static DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
/// The component to look for when waiting for redirects
const REDIRECT_COMPONENT: Components = Components::Redirector(Redirector::GetServerInstance);

/// Handles dealing with a redirector client
///
/// `stream`   The stream to the client
/// `addr`     The client address
/// `instance` The server instance information
async fn handle_client(stream: TcpStream, addr: SocketAddr) -> BlazeResult<()> {
    let mut shutdown = GlobalState::shutdown();

    let mut server = match BlazeStream::new(stream, StreamMode::Server).await {
        Ok(stream) => stream,
        Err(_) => {
            error!("Unable to establish SSL connection within redirector");
            return Ok(());
        }
    };

    loop {
        let (component, packet) = select! {
            // Attempt to read packets from the stream
            result = Packet::read_blaze_typed::<Components, TcpStream>(&mut stream) => result,
            // Shutdown hook to ensure we don't keep trying to read after shutdown
            _ = shutdown.changed() => { break; }
            // If the timeout completes before the redirect is complete the
            // request is considered over and terminates
            _ = sleep(DEFAULT_TIMEOUT) => { break; }
        }?;

        if component == REDIRECT_COMPONENT {
            debug!("Redirecting client (Addr: {addr:?})");

            let host = constants::EXTERNAL_HOST;
            let port = env::from_env(env::MAIN_PORT);
            let instance = InstanceDetails {
                net: InstanceNet::from((host.to_string(), port)),
                secure: false,
            };

            let response = Packet::response(&packet, instance);
            response.write_blaze(&mut stream)?;
            stream.flush().await?;
            break;
        } else {
            let response = Packet::response_empty(&packet);
            response.write_blaze(&mut stream)?;
            stream.flush().await?;
        }
    }

    Ok(())
}
