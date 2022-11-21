//! Module for the Redirector server which handles redirecting the clients
//! to the correct address for the main server.

use core::blaze::components::{Components, Redirector};
use core::{env, GlobalStateArc};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use blaze_pk::packet::Packet;

use blaze_ssl_async::stream::{BlazeStream, StreamMode};
use log::{debug, error, info};
use tokio::net::{TcpListener, TcpStream};
use tokio::select;
use tokio::sync::watch;
use tokio::time::sleep;

pub mod shared;

use self::shared::{InstanceType, RedirectorInstance};

/// Starts the Redirector server using the provided global state
///
/// `global` The global state
pub async fn start_server(global: GlobalStateArc) {
    let listener = {
        let port = env::from_env(env::REDIRECTOR_PORT);
        match TcpListener::bind(("0.0.0.0", port)).await {
            Ok(value) => {
                info!("Started Redirector Server on (Port: {port})");
                value
            }
            Err(err) => {
                error!(
                    "Failed to bind redirector server (Port: {}): {:?}",
                    port, err
                );
                panic!();
            }
        }
    };

    let instance = {
        let host = env::env(env::EXT_HOST);
        let port = env::from_env(env::MAIN_PORT);

        let ty = InstanceType::from_host(host);

        RedirectorInstance::new(ty, port)
    };

    let instance = Arc::new(instance);

    let mut shutdown = global.shutdown.clone();
    loop {
        select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, addr)) => {
                        tokio::spawn(handle_client(
                            stream,
                            addr,
                            instance.clone(),
                            shutdown.clone()
                        ));
                    },
                    Err(err) => {
                        error!("Error occurred while accepting connections: {:?}", err);
                    }
                }
            }
            _ = shutdown.changed() => {
                info!("Stopping redirector server listener from shutdown trigger.");
                break;
            }
        }
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
/// `shutdown` Async safely shutdown reciever
async fn handle_client(
    stream: TcpStream,
    addr: SocketAddr,
    instance: Arc<RedirectorInstance>,
    mut shutdown: watch::Receiver<()>,
) {
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
