//! Module for the Redirector server which handles redirecting the clients
//! to the correct address for the main server.

use core::blaze::components::{Components, Redirector};
use core::{env, state::GlobalState};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use blaze_pk::packet::Packet;

use blaze_ssl_async::stream::{BlazeStream, StreamMode};
use log::{debug, error};
use tokio::net::TcpStream;
use tokio::select;
use tokio::time::sleep;
use utils::net::{accept_stream, listener};

pub mod codec;

use self::codec::{InstanceType, RedirectorInstance};

/// Starts the Redirector server
pub async fn start_server() {
    // The server details of the instance clients should
    // connect to. In this case its the main server details
    let instance = {
        let host = env::env(env::EXT_HOST);
        let port = env::from_env(env::MAIN_PORT);
        let ty = InstanceType::from_host(host);
        RedirectorInstance::new(ty, port)
    };
    let instance = Arc::new(instance);
    let listener = listener("Redirector", env::from_env(env::REDIRECTOR_PORT)).await;
    let mut shutdown = GlobalState::shutdown();
    while let Some((stream, addr)) = accept_stream(&listener, &mut shutdown).await {
        tokio::spawn(handle_client(stream, addr, instance.clone()));
    }
}

/// The timeout before idle redirector connections are terminated
/// (1 minutes before disconnect timeout)
static DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// Handles dealing with a redirector client
///
/// `stream`   The stream to the client
/// `addr`     The client address
/// `instance` The server instance information
async fn handle_client(stream: TcpStream, addr: SocketAddr, instance: Arc<RedirectorInstance>) {
    let mut shutdown = GlobalState::shutdown();
    let mut stream = match BlazeStream::new(stream, StreamMode::Server).await {
        Ok(stream) => stream,
        Err(err) => {
            error!("Failed to accept connection: {err:?}");
            return;
        }
    };

    loop {
        let result = select! {
            result = Packet::read_blaze_typed::<Components, TcpStream>(&mut stream) => result,
            _ = shutdown.changed() => {
                break;
            }
            _ = sleep(DEFAULT_TIMEOUT) => { break; }
        };

        let Ok((component, packet)) = result else {
            error!("Failed to read packet from redirector client");
            break;
        };

        if component != Components::Redirector(Redirector::GetServerInstance) {
            let response = Packet::response_empty(&packet);
            if let Err(_) = response.write_blaze(&mut stream) {
                break;
            }
            if let Err(_) = stream.flush().await {
                break;
            }
        } else {
            debug!("Redirecting client (Addr: {addr:?})");
            let response = Packet::response(&packet, &*instance);
            response.write_blaze(&mut stream).ok();
            stream.flush().await.ok();
            break;
        }
    }
}
