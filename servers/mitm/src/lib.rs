//! Module for the Redirector server which handles redirecting the clients
//! to the correct address for the main server.

use core::blaze::components::Components;
use core::{env, GlobalStateArc};
use std::net::SocketAddr;

use blaze_pk::codec::Reader;
use blaze_pk::packet::{Packet, PacketType};

use blaze_pk::tag::Tag;
use log::{debug, error, info, log_enabled};
use tokio::net::{TcpListener, TcpStream};
use tokio::select;

/// Starts the Redirector server using the provided global state
///
/// `global` The global state
pub async fn start_server(global: GlobalStateArc) {
    let listener = {
        let port = env::from_env(env::MAIN_PORT);
        match TcpListener::bind(("0.0.0.0", port)).await {
            Ok(value) => {
                info!("Started MITM Server on (Port: {port})");
                value
            }
            Err(err) => {
                error!("Failed to bind MITM server (Port: {}): {:?}", port, err);
                panic!();
            }
        }
    };

    let mut shutdown = global.shutdown.clone();
    loop {
        select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, addr)) => {
                        tokio::spawn(handle_client(
                            stream,
                            addr,
                            global.clone(),
                        ));
                    },
                    Err(err) => {
                        error!("Error occurred while accepting connections: {:?}", err);
                    }
                }
            }
            _ = shutdown.changed() => {
                info!("Stopping MITM server listener from shutdown trigger.");
                break;
            }
        }
    }
}

/// Handles dealing with a redirector client
///
/// `stream`   The stream to the client
/// `addr`     The client address
/// `instance` The server instance information
/// `shutdown` Async safely shutdown reciever
async fn handle_client(mut client: TcpStream, _addr: SocketAddr, global: GlobalStateArc) {
    let mut shutdown = global.shutdown.clone();

    let Some(retriever) = global.retriever.as_ref() else {
        error!("Server is in MITM mode but was unable to connect to the official servers. Denying connection from client");
        return;
    };

    let Some(mut server) = retriever.stream().await else {
        error!("Unable to connection to official server for MITM connection");
        return;
    };

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
