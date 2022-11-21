//! Module for the Redirector server which handles redirecting the clients
//! to the correct address for the main server.

use core::blaze::components::Components;
use core::retriever::Retriever;
use core::{env, state::GlobalState};
use std::net::SocketAddr;

use blaze_pk::codec::Reader;
use blaze_pk::packet::{Packet, PacketType};

use blaze_pk::tag::Tag;
use log::{debug, error, log_enabled};
use tokio::net::TcpStream;
use tokio::select;
use utils::net::{accept_stream, listener};

/// Starts the MITM server
pub async fn start_server() {
    let Some(retriever) = GlobalState::retriever() else {
        error!("Server is in MITM mode but was unable to connect to the official servers. Stopping server.");
        panic!();
    };

    let listener = listener("MITM", env::from_env(env::MAIN_PORT)).await;
    let mut shutdown = GlobalState::shutdown();
    while let Some((stream, addr)) = accept_stream(&listener, &mut shutdown).await {
        tokio::spawn(handle_client(stream, addr, retriever));
    }
}

/// Handles dealing with a redirector client
///
/// `stream`   The stream to the client
/// `addr`     The client address
/// `instance` The server instance information
/// `shutdown` Async safely shutdown reciever
async fn handle_client(mut client: TcpStream, addr: SocketAddr, retriever: &'static Retriever) {
    let Some(mut server) = retriever.stream().await else {
        error!("Unable to connection to official server for MITM connection: (Addr: {addr})");
        return;
    };
    let mut shutdown = GlobalState::shutdown();

    loop {
        select! {
            result = Packet::read_async_typed::<Components, TcpStream>(&mut client) => {
                let Ok((component, packet)) = result else { break; };
                log_packet(component, &packet, "From Client");
                if let Err(_) = packet.write_blaze(&mut server) {
                    break;
                }
                if let Err(_) = server.flush().await {
                    break;
                }
            }
            result = Packet::read_blaze_typed::<Components, TcpStream>(&mut server) => {
                let Ok((component, packet)) = result else { break; };
                log_packet(component, &packet, "From Server");
                if let Err(_) =  packet.write_async(&mut client).await {
                    break;
                }
            }
            _ = shutdown.changed() => {
                break;
            }

        };
    }
}

fn log_packet(component: Components, packet: &Packet, direction: &str) {
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

    match header.ty {
        PacketType::Notify => {}
        _ => {
            message.push_str(&format!("\nID: {}", header.id));
        }
    }

    let mut reader = Reader::new(&packet.contents);
    let mut out = String::new();
    out.push_str("{\n");
    match Tag::stringify(&mut reader, &mut out, 1) {
        Ok(_) => {}
        Err(err) => {
            message.push_str("\nExtra: Content was malformed");
            message.push_str(&format!("\nError: {:?}", err));
            message.push_str(&format!("\nPartial Content: {}", out));
            debug!("{}", message);
            return;
        }
    };
    if out.len() == 2 {
        // Remove new line if nothing else was appended
        out.pop();
    }
    out.push('}');
    message.push_str(&format!("\nContent: {}", out));
    debug!("{}", message);
}
