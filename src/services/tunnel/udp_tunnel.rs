use crate::services::sessions::Sessions;
use log::{debug, error};
use parking_lot::RwLock;
use pocket_relay_udp_tunnel::{deserialize_message, serialize_message, TunnelMessage};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    net::UdpSocket,
    task::JoinSet,
    time::{interval_at, Instant, MissedTickBehavior},
};

use super::mappings::{TunnelData, TunnelMappings};
use super::{TunnelId, TunnelService};

/// The port bound on clients representing the host player within the socket pool
pub const _TUNNEL_HOST_LOCAL_PORT: u16 = 42132;

pub async fn start_udp_tunnel(
    tunnel_addr: SocketAddr,
    service: Arc<TunnelService>,
) -> std::io::Result<()> {
    let socket = UdpSocket::bind(tunnel_addr).await?;
    let socket = Arc::new(socket);

    debug!("started tunneling server {tunnel_addr}");

    // Spawn the task to handle accepting messages
    tokio::spawn(accept_messages(service.clone(), socket.clone()));

    // Spawn task to keep connections alive
    tokio::spawn(keep_alive(service, socket));

    Ok(())
}

/// Reads inbound messages from the tunnel service
pub async fn accept_messages(service: Arc<TunnelService>, socket: Arc<UdpSocket>) {
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

        let service = service.clone();
        let socket = socket.clone();

        // Handle the message in its own task
        tokio::spawn(async move {
            service
                .udp
                .handle_message(socket, tunnel_id, packet.message, addr)
                .await;
        });
    }
}

/// Delay between each keep-alive packet
const KEEP_ALIVE_DELAY: Duration = Duration::from_secs(10);

/// When this duration elapses between keep-alive checks for a connection
/// the connection is considered to be dead (4 missed keep-alive check intervals)
const KEEP_ALIVE_TIMEOUT: Duration = Duration::from_secs(KEEP_ALIVE_DELAY.as_secs() * 4);

/// Background task that sends out keep alive messages to all the sockets connected
/// to the tunnel system. Removes inactive and dead connections
pub async fn keep_alive(service: Arc<TunnelService>, socket: Arc<UdpSocket>) {
    // Task set for keep alive tasks
    let mut send_task_set = JoinSet::new();

    // Create the interval to track keep alive pings
    let keep_alive_start = Instant::now() + KEEP_ALIVE_DELAY;
    let mut keep_alive_interval = interval_at(keep_alive_start, KEEP_ALIVE_DELAY);

    keep_alive_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        // Wait for the next keep-alive tick
        keep_alive_interval.tick().await;

        let now = Instant::now();

        // Read the tunnels of all current tunnels
        let tunnels: Vec<(TunnelId, TunnelData<SocketAddr>)> =
            { service.udp.mappings.read().tunnel_data() };

        // Don't need to tick if theres no tunnels available
        if tunnels.is_empty() {
            continue;
        }

        let mut expired_tunnels: Vec<TunnelId> = Vec::new();

        // Send out keep-alive messages for any tunnels that aren't expired
        for (tunnel_id, data) in tunnels {
            let last_alive = data.last_alive.duration_since(now);
            if last_alive > KEEP_ALIVE_TIMEOUT {
                expired_tunnels.push(tunnel_id);
                continue;
            }

            let buffer = serialize_message(tunnel_id, &TunnelMessage::KeepAlive);

            // Spawn the task to send the keep-alive message
            send_task_set.spawn({
                let socket = socket.clone();

                async move { socket.send_to(&buffer, data.handle).await }
            });
        }

        // Join all keep alive tasks
        while send_task_set.join_next().await.is_some() {}

        // Drop any tunnel connections that have passed acceptable keep-alive bounds
        if !expired_tunnels.is_empty() {
            let mappings = &mut *service.udp.mappings.write();

            for tunnel_id in expired_tunnels {
                mappings.dissociate_tunnel(tunnel_id);
            }
        }
    }
}

/// UDP tunneling service
pub struct UdpTunnelService {
    /// Tunneling mapping data
    pub mappings: RwLock<TunnelMappings<SocketAddr>>,
    /// Access to the session service for exchanging
    /// association tokens
    sessions: Arc<Sessions>,
}

impl UdpTunnelService {
    pub fn new(sessions: Arc<Sessions>) -> Self {
        Self {
            mappings: RwLock::new(TunnelMappings::default()),
            sessions,
        }
    }

    /// Handles processing a message received through the tunnel
    async fn handle_message(
        &self,
        socket: Arc<UdpSocket>,
        tunnel_id: u32,
        msg: TunnelMessage,
        addr: SocketAddr,
    ) {
        // Only process tunnels with known IDs
        if tunnel_id != u32::MAX {
            // Store the updated tunnel address
            self.mappings.write().update_tunnel_handle(tunnel_id, addr);
        }

        match msg {
            TunnelMessage::Initiate { association_token } => {
                let association = match self.sessions.verify_assoc_token(&association_token) {
                    Ok(value) => value,
                    Err(err) => {
                        error!("client send invalid association token: {}", err);
                        return;
                    }
                };

                // Store the tunnel mapping
                let tunnel_id = self.mappings.write().insert_tunnel(
                    association,
                    TunnelData {
                        handle: addr,
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

                _ = socket.send_to(&buffer, addr).await;
            }
            TunnelMessage::Initiated { .. } => {
                // Server shouldn't be receiving this message... ignore it
            }
            TunnelMessage::Forward { index, message } => {
                // Get the path through the tunnel
                let (target_addr, index) = {
                    match self.mappings.read().get_tunnel_route(tunnel_id, index) {
                        Some(value) => value,
                        // Don't have a tunnel to send the message through
                        None => {
                            return;
                        }
                    }
                };

                let buffer =
                    serialize_message(tunnel_id, &TunnelMessage::Forward { index, message });

                _ = socket.send_to(&buffer, target_addr).await;
            }
            TunnelMessage::KeepAlive => {
                // Update tunnel last alive time
                self.mappings
                    .write()
                    .update_tunnel_last_alive(tunnel_id, Instant::now());
            }
        }
    }
}
