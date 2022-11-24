//! Module for the Redirector server which handles redirecting the clients
//! to the correct address for the main server.

use core::blaze::append_packet_decoded;
use core::blaze::components::Components;
use core::blaze::errors::{BlazeError, HandleResult};
use core::retriever::Retriever;
use core::{env, state::GlobalState};

use blaze_pk::packet::{Packet, PacketType};

use log::{debug, error, log_enabled};
use tokio::net::TcpStream;
use tokio::select;
use utils::net::{accept_stream, listener};

/// Starts the MITM server. This server is responsible for creating a sort of
/// proxy between this server and the official servers. All packets send and
/// recieved by this server are forwarded to the official servers and are logged
/// using the debug logging.
pub async fn start_server() {
    // MITM server is unable to start if the retriever is disabled or fails to connect
    let Some(retriever) = GlobalState::retriever() else {
        error!("Server is in MITM mode but was unable to connect to the official servers. Stopping server.");
        panic!();
    };

    let listener = listener("MITM", env::from_env(env::MAIN_PORT)).await;
    let mut shutdown = GlobalState::shutdown();
    while let Some((stream, addr)) = accept_stream(&listener, &mut shutdown).await {
        tokio::spawn(async move {
            if let Err(err) = handle_client(stream, retriever).await {
                error!("Unable to handle MITM (Addr: {addr}): {err}");
            }
        });
    }
}

/// Handles dealing with a redirector client
///
/// `stream`   The stream to the client
/// `addr`     The client address
/// `instance` The server instance information
/// `shutdown` Async safely shutdown reciever
async fn handle_client(mut client: TcpStream, retriever: &'static Retriever) -> HandleResult {
    let mut server = retriever
        .stream()
        .await
        .ok_or_else(|| BlazeError::Other("Unable to connection to official server"))?;

    let mut shutdown = GlobalState::shutdown();

    loop {
        select! {
            // Read packets coming from the client
            result = Packet::read_async_typed::<Components, TcpStream>(&mut client) => {
                let (component, packet) = result?;
                debug_log_packet(component, &packet, "From Client");
                packet.write_blaze(&mut server)?;
                server.flush().await?;
            }
            // Read packets from the official server
            result = Packet::read_blaze_typed::<Components, TcpStream>(&mut server) => {
                let (component, packet) = result?;
                debug_log_packet(component, &packet, "From Server");
                packet.write_async(&mut client).await?;
            }
            // Shutdown hook to ensure we don't keep trying to read after shutdown
            _ = shutdown.changed() => {   break;  }
        };
    }

    Ok(())
}

/// Logs the contents of the provided packet to the debug output along with
/// the header information.
///
/// `component` The component for the packet routing
/// `packet`    The packet that is being logged
/// `direction` The direction name for the packet
fn debug_log_packet(component: Components, packet: &Packet, direction: &str) {
    // Skip if debug logging is disabled
    if !log_enabled!(log::Level::Debug) {
        return;
    }
    let header = &packet.header;
    let mut message = String::new();
    message.push_str("\nRecieved Packet ");
    message.push_str(direction);
    message.push_str(&format!("\nComponent: {:?}", component));
    message.push_str(&format!("\nType: {:?}", header.ty));
    if header.ty != PacketType::Notify {
        message.push_str("\nID: ");
        message.push_str(&header.id.to_string());
    }
    append_packet_decoded(packet, &mut message);
    debug!("{}", message);
}
