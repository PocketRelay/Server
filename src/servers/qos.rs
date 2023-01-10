//! Module for the Redirector server which handles redirecting the clients
//! to the correct address for the main server.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::{env, utils::net::public_address};
use log::{error, info};
use tokio::net::UdpSocket;

pub async fn start_server() {
    let socket = {
        let port = env::from_env(env::QOS_PORT);
        match UdpSocket::bind(("0.0.0.0", port)).await {
            Ok(value) => {
                info!("Started QOS server (Port: {})", port);
                value
            }
            Err(_) => {
                error!("Failed to bind QOS server (Port: {})", port);
                panic!()
            }
        }
    };

    // Buffer for the heading portion of the incoming message
    let mut buffer = [0u8; 20];
    // Buffer for the output message
    let mut output = [0u8; 30];

    loop {
        let (_, addr) = match socket.recv_from(&mut buffer).await {
            Ok(value) => value,
            Err(err) => {
                error!("Error while recieving QOS message: {:?}", err);
                continue;
            }
        };

        let address = match get_address(&addr).await {
            Some(value) => value,
            None => {
                error!("Client address was unable to be found");
                continue;
            }
        };

        let address = address.octets();

        // Copy the heading from the read buffer
        output[..20].copy_from_slice(&buffer);

        // Copy the address bytes
        output[20..24].copy_from_slice(&address);

        // Fill remaining contents
        output[24..].copy_from_slice(&[246, 162, 0, 0, 0, 0]);

        // Send output response
        match socket.send_to(&output, addr).await {
            Ok(_) => {}
            Err(err) => {
                error!("Unable to send response to QOS request: {:?}", err);
            }
        }
    }
}

async fn get_address(addr: &SocketAddr) -> Option<Ipv4Addr> {
    let ip = addr.ip();
    if let IpAddr::V4(value) = ip {
        // Attempt to lookup machine public address to use
        if value.is_loopback() || value.is_private() {
            if let Some(public_addr) = public_address().await {
                return Some(public_addr);
            }
        }
        return Some(value);
    }
    None
}
