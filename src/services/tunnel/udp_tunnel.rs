use crate::services::sessions::Sessions;
use futures_util::{stream::FuturesUnordered, Stream};
use log::{debug, error};
use pocket_relay_udp_tunnel::{deserialize_message, serialize_message, TunnelMessage};
use std::{future::poll_fn, net::SocketAddr, sync::Arc, task::Poll, time::Duration};
use tokio::{
    net::UdpSocket,
    time::{interval_at, Instant, MissedTickBehavior},
};

use super::{
    mappings::{TunnelData, TunnelHandle},
    TunnelBuffer, UdpTunnelForwardRx,
};
use super::{TunnelId, TunnelService};

/// The port bound on clients representing the host player within the socket pool
pub const _TUNNEL_HOST_LOCAL_PORT: u16 = 42132;

/// Message to send an already encoded message to a specific
/// UDP address through the UDP tunnel
pub struct UdpTunnelMessage {
    // Payload
    pub buffer: Vec<u8>,
    /// Address to send to
    pub target_address: SocketAddr,
}

pub async fn start_udp_tunnel(
    tunnel_addr: SocketAddr,
    service: Arc<TunnelService>,
    sessions: Arc<Sessions>,
    udp_forward_rx: UdpTunnelForwardRx,
) -> std::io::Result<()> {
    let socket = UdpSocket::bind(tunnel_addr).await?;

    debug!("started tunneling server {tunnel_addr}");

    tokio::spawn(async move {
        let service = &*service;
        let sessions = &*sessions;
        let socket = &socket;

        // Accept messages future
        let accept_future = accept_messages(service, sessions, socket);

        // Keep alive future
        let keep_alive_future = keep_alive(service);

        // Forwarding future
        let forward_future = forward_messages(socket, udp_forward_rx);

        tokio::join!(accept_future, keep_alive_future, forward_future);
    });

    Ok(())
}

pub async fn forward_messages(socket: &UdpSocket, mut rx: UdpTunnelForwardRx) {
    let mut futures = FuturesUnordered::new();
    let mut futures = std::pin::pin!(futures);

    poll_fn(|cx| {
        // Poll new event execution
        while let Poll::Ready(result) = rx.poll_recv(cx) {
            let message = match result {
                Some(value) => value,

                // All channels have been closed and the app is likely shutting down,
                // finish the future and stop processing
                None => return Poll::Ready(()),
            };

            // Push send future
            futures.push(async move {
                // Move required variables into the future
                let message = message;

                // Send message
                _ = socket
                    .send_to(&message.buffer, message.target_address)
                    .await;
            });
        }

        // Poll completions until no more ready
        while let Poll::Ready(Some(_)) = futures.as_mut().poll_next(cx) {}

        Poll::Pending
    })
    .await;
}

/// Reads inbound messages from the tunnel service
pub async fn accept_messages(service: &TunnelService, sessions: &Sessions, socket: &UdpSocket) {
    // Buffer to recv messages
    let mut buffer = [0; u16::MAX as usize];

    loop {
        // Receive the message bytes
        let (size, addr) = match socket.recv_from(&mut buffer).await {
            Ok(value) => value,
            Err(err) => {
                if let Some(error_code) = err.raw_os_error() {
                    // Ignore "An existing connection was forcibly closed by the remote host."
                    // this happens when we tried to send a packet to a closed connection.
                    // error happens here instead of the sending portion for some reason
                    if error_code == 10054 {
                        continue;
                    }
                }

                error!("failed to recv message: {err}");
                continue;
            }
        };

        let buffer = &buffer[0..size];

        // Deserialize the message
        let packet = match deserialize_message(buffer) {
            Ok(value) => value,
            Err(err) => {
                error!("failed to deserialize packet: {}", err);
                continue;
            }
        };

        let tunnel_id = packet.header.tunnel_id;

        // Handle the message in its own task
        handle_message(service, sessions, tunnel_id, packet.message, addr);
    }
}

/// Delay between each keep-alive packet
const KEEP_ALIVE_DELAY: Duration = Duration::from_secs(10);

/// When this duration elapses between keep-alive checks for a connection
/// the connection is considered to be dead (4 missed keep-alive check intervals)
const KEEP_ALIVE_TIMEOUT: Duration = Duration::from_secs(KEEP_ALIVE_DELAY.as_secs() * 4);

/// Background task that sends out keep alive messages to all the sockets connected
/// to the tunnel system. Removes inactive and dead connections
pub async fn keep_alive(service: &TunnelService) {
    // Create the interval to track keep alive pings
    let keep_alive_start = Instant::now() + KEEP_ALIVE_DELAY;
    let mut keep_alive_interval = interval_at(keep_alive_start, KEEP_ALIVE_DELAY);

    keep_alive_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        // Wait for the next keep-alive tick
        keep_alive_interval.tick().await;

        let now = Instant::now();

        // Read the tunnels of all current tunnels
        let tunnels: Vec<(TunnelId, SocketAddr, Instant)> = {
            service
                .mappings
                .read()
                .tunnel_data()
                .into_iter()
                // Filter to only the UDP handles
                .filter_map(|(tunnel_id, data)| {
                    let handle = match data.handle {
                        // Only perform keep alive
                        TunnelHandle::Udp(socket_addr) => socket_addr,
                        TunnelHandle::Http(_) => return None,
                    };

                    Some((tunnel_id, handle, data.last_alive))
                })
                .collect()
        };

        // Don't need to tick if theres no tunnels available
        if tunnels.is_empty() {
            continue;
        }

        let mut expired_tunnels: Vec<TunnelId> = Vec::new();

        // Send out keep-alive messages for any tunnels that aren't expired
        for (tunnel_id, target_address, last_alive) in tunnels {
            let last_alive = last_alive.duration_since(now);
            if last_alive > KEEP_ALIVE_TIMEOUT {
                expired_tunnels.push(tunnel_id);
                continue;
            }

            let buffer = serialize_message(tunnel_id, &TunnelMessage::KeepAlive);

            // Send keep alive message
            _ = service.udp_tx.send(UdpTunnelMessage {
                buffer,
                target_address,
            });
        }

        // Drop any tunnel connections that have passed acceptable keep-alive bounds
        if !expired_tunnels.is_empty() {
            let mappings = &mut *service.mappings.write();

            for tunnel_id in expired_tunnels {
                mappings.dissociate_tunnel(tunnel_id);
            }
        }
    }
}

/// Handles processing a message received through the tunnel
fn handle_message(
    service: &TunnelService,
    sessions: &Sessions,
    tunnel_id: u32,
    msg: TunnelMessage,
    addr: SocketAddr,
) {
    // Only process tunnels with known IDs
    if tunnel_id != u32::MAX {
        // Store the updated tunnel address
        service
            .mappings
            .write()
            .update_tunnel_handle(tunnel_id, TunnelHandle::Udp(addr));
    }

    match msg {
        TunnelMessage::Initiate { association_token } => {
            let association = match sessions.verify_assoc_token(&association_token) {
                Ok(value) => value,
                Err(err) => {
                    error!("client send invalid association token: {}", err);
                    return;
                }
            };

            // Store the tunnel mapping
            let tunnel_id = service.mappings.write().insert_tunnel(
                association,
                TunnelData {
                    handle: TunnelHandle::Udp(addr),
                    association,
                    last_alive: Instant::now(),
                },
            );

            let tunnel_id = match tunnel_id {
                Some(value) => value,
                // Cannot allocate the tunnel an ID
                None => {
                    error!("failed to allocate a tunnel ID: exhausted");
                    return;
                }
            };

            let buffer = serialize_message(tunnel_id, &TunnelMessage::Initiated { tunnel_id });

            _ = service.udp_tx.send(UdpTunnelMessage {
                buffer,
                target_address: addr,
            });
        }
        TunnelMessage::Initiated { .. } => {
            // Server shouldn't be receiving this message... ignore it
        }
        TunnelMessage::Forward { index, message } => {
            service.send_to(tunnel_id, TunnelBuffer::Owned(message), index);
        }
        TunnelMessage::KeepAlive => {
            // Update tunnel last alive time
            service
                .mappings
                .write()
                .update_tunnel_last_alive(tunnel_id, Instant::now());
        }
    }
}
