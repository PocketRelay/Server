use crate::services::sessions::Sessions;
use log::{debug, error};
use pocket_relay_udp_tunnel::{TunnelMessage, deserialize_message, serialize_message};
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::task::ready;
use std::{net::SocketAddr, sync::Arc, task::Poll};
use tokio::io::ReadBuf;
use tokio::{net::UdpSocket, time::Instant};

use super::TunnelService;
use super::{
    TunnelBuffer, UdpTunnelForwardRx,
    mappings::{TunnelData, TunnelHandle},
};

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

    tokio::spawn(UdpTunnelFuture {
        service,
        sessions,
        socket,
        buffer: [0; u16::MAX as usize],
        write_queue: VecDeque::new(),
        rx: udp_forward_rx,
    });

    Ok(())
}

/// Future which handles processing UDP tunnel related tasks.
pub struct UdpTunnelFuture {
    /// Tunnel service for managing tunnels
    service: Arc<TunnelService>,

    /// Session service for verifying association tokens
    sessions: Arc<Sessions>,

    /// Socket for sending and receiving messages
    socket: UdpSocket,

    /// Buffer for reading received datagram packets
    buffer: [u8; u16::MAX as usize],

    /// Receiver for messages to write
    rx: UdpTunnelForwardRx,

    /// Queue for messages that need to be written
    write_queue: VecDeque<UdpTunnelMessage>,
}

impl UdpTunnelFuture {
    /// Poll receiving messages from the UDP socket
    fn poll_recv_message(&mut self, cx: &mut std::task::Context<'_>) -> Poll<()> {
        let mut buffer = ReadBuf::new(&mut self.buffer);
        let addr = match ready!(self.socket.poll_recv_from(cx, &mut buffer)) {
            Ok(value) => value,
            Err(err) => {
                if let Some(error_code) = err.raw_os_error() {
                    // Ignore "An existing connection was forcibly closed by the remote host."
                    // this happens when we tried to send a packet to a closed connection.
                    // error happens here instead of the sending portion for some reason
                    if error_code == 10054 {
                        return Poll::Ready(());
                    }
                }

                error!("failed to recv message: {err}");
                return Poll::Ready(());
            }
        };

        let buffer = buffer.filled();

        // Deserialize the message
        let packet = match deserialize_message(buffer) {
            Ok(value) => value,
            Err(err) => {
                error!("failed to deserialize packet: {err}");
                return Poll::Ready(());
            }
        };

        let tunnel_id = packet.header.tunnel_id;

        // Handle the message in its own task
        handle_message(
            &self.service,
            &self.sessions,
            tunnel_id,
            packet.message,
            addr,
        );
        Poll::Ready(())
    }

    /// Poll messages from the send channel and place the send future onto the
    /// futures list
    fn poll_forward_message(&mut self, cx: &mut std::task::Context<'_>) -> Poll<()> {
        let message = match ready!(self.rx.poll_recv(cx)) {
            Some(value) => value,

            // All channels have been closed and the app is likely shutting down,
            // finish the future and stop processing
            None => return Poll::Ready(()),
        };

        // Push message to the write queue
        self.write_queue.push_back(message);

        Poll::Ready(())
    }

    fn poll_write_message(&mut self, cx: &mut std::task::Context<'_>) -> Poll<()> {
        let message = match self.write_queue.front() {
            Some(value) => value,

            // Waiting on a new packet to write
            None => return Poll::Pending,
        };

        // Attempt to send the message
        _ = ready!(
            self.socket
                .poll_send_to(cx, &message.buffer, message.target_address)
        );

        // Remove the sent message from the queue
        _ = self.write_queue.pop_front();

        Poll::Ready(())
    }
}

impl Future for UdpTunnelFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        // Poll incoming data
        while this.poll_recv_message(cx).is_ready() {}

        // Poll outgoing data
        while this.poll_forward_message(cx).is_ready() {}

        // Poll outgoing writes
        while this.poll_write_message(cx).is_ready() {}

        Poll::Pending
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
        service.update_tunnel_handle(tunnel_id, TunnelHandle::Udp(addr));
    }

    match msg {
        TunnelMessage::Initiate { association_token } => {
            let association = match sessions.verify_assoc_token(&association_token) {
                Ok(value) => value,
                Err(err) => {
                    error!("client send invalid association token: {err}");
                    return;
                }
            };

            // Store the tunnel mapping
            let tunnel_id = service.insert_tunnel(
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

            debug!("Session UDP tunnel connected (ASSOC: {association:?}, TUNNEL_ID: {tunnel_id})");

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
            service.update_tunnel_last_alive(tunnel_id, Instant::now());
        }
    }
}
