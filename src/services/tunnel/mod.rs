use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use http_tunnel::HttpTunnelMessage;
use mappings::{PoolKey, TunnelData, TunnelHandle, TunnelMappings};
use parking_lot::RwLock;
use tokio::{
    sync::mpsc,
    time::{interval_at, Instant, MissedTickBehavior},
};
use udp_tunnel::UdpTunnelMessage;

use crate::utils::types::GameID;

use super::sessions::AssociationId;

pub mod http_tunnel;
pub mod mappings;
pub mod udp_tunnel;

/// ID for a tunnel
pub type TunnelId = u32;
/// Index into a pool of tunnels
pub type PoolIndex = u8;
/// ID of a pool
pub type PoolId = GameID;

pub type UdpTunnelForwardTx = mpsc::UnboundedSender<UdpTunnelMessage>;
pub type UdpTunnelForwardRx = mpsc::UnboundedReceiver<UdpTunnelMessage>;

pub struct TunnelService {
    /// Mappings between tunnel IDs and the tunnel itself
    mappings: RwLock<TunnelMappings>,

    // Sender for udp tunnel messages to go out
    pub udp_tx: UdpTunnelForwardTx,
}

pub enum TunnelBuffer {
    /// UDP tunnel uses owned [Vec] of bytes
    Owned(Vec<u8>),
    /// HTTP tunnel uses shared [Bytes]
    Shared(Bytes),
}

impl TunnelService {
    pub fn new() -> (Self, UdpTunnelForwardRx) {
        let (tx, rx) = mpsc::unbounded_channel();

        (
            Self {
                mappings: Default::default(),
                udp_tx: tx,
            },
            rx,
        )
    }

    pub fn associate_pool(
        &self,
        association: AssociationId,
        pool_id: PoolId,
        pool_index: PoolIndex,
    ) {
        let mappings = &mut *self.mappings.write();
        let tunnel_id = {
            match mappings.get_association_tunnel(&association) {
                Some(value) => value,
                None => return,
            }
        };

        mappings.associate_pool(tunnel_id, pool_id, pool_index);
    }

    pub fn dissociate_pool(&self, pool_id: PoolId, pool_index: PoolIndex) {
        self.mappings.write().dissociate_pool(pool_id, pool_index);
    }

    pub fn insert_tunnel(
        &self,
        association: AssociationId,
        tunnel: TunnelData,
    ) -> Option<TunnelId> {
        self.mappings.write().insert_tunnel(association, tunnel)
    }

    pub fn update_tunnel_handle(&self, tunnel_id: TunnelId, handle: TunnelHandle) {
        self.mappings
            .write()
            .update_tunnel_handle(tunnel_id, handle);
    }

    pub fn update_tunnel_last_alive(&self, tunnel_id: TunnelId, last_alive: Instant) {
        self.mappings
            .write()
            .update_tunnel_last_alive(tunnel_id, last_alive);
    }

    pub fn send_to(
        &self,

        // Sender details
        from_tunnel_id: TunnelId,

        // Payload
        buffer: TunnelBuffer,

        // Target details
        to_index: u8,
    ) {
        let mappings = self.mappings.read();

        // Get our tunnels current pool data
        let (pool_id, pool_index) = match mappings.get_tunnel_pool_key(from_tunnel_id) {
            Some(value) => value,

            // Player is not apart of any game pool
            None => return,
        };

        // Pool key for our target tunnel
        let target_pool_key = PoolKey::new(pool_id, to_index);

        // Get the target tunnel within our pool
        let target = match mappings.get_tunnel_by_pool_key(&target_pool_key) {
            Some(value) => value,

            // Target player doesn't have a tunnel
            None => return,
        };

        // Forward message to target tunnel
        match target {
            TunnelHandle::Udp(socket_addr) => {
                let message = match buffer {
                    TunnelBuffer::Owned(items) => items,
                    TunnelBuffer::Shared(bytes) => bytes.into(),
                };

                let buffer: Vec<u8> = pocket_relay_udp_tunnel::serialize_message(
                    from_tunnel_id,
                    &pocket_relay_udp_tunnel::TunnelMessage::Forward {
                        index: pool_index,
                        message,
                    },
                );

                _ = self.udp_tx.send(UdpTunnelMessage {
                    buffer,
                    target_address: *socket_addr,
                });
            }
            TunnelHandle::Http(tunnel_handle) => {
                let message = match buffer {
                    TunnelBuffer::Owned(items) => items.into(),
                    TunnelBuffer::Shared(bytes) => bytes,
                };

                _ = tunnel_handle.tx.send(HttpTunnelMessage {
                    index: pool_index,
                    message,
                });
            }
        }
    }
}

/// Delay between each keep-alive packet
const KEEP_ALIVE_DELAY: Duration = Duration::from_secs(10);

/// When this duration elapses between keep-alive checks for a connection
/// the connection is considered to be dead (4 missed keep-alive check intervals)
const KEEP_ALIVE_TIMEOUT: Duration = Duration::from_secs(KEEP_ALIVE_DELAY.as_secs() * 4);

/// Background task that sends out keep alive messages to all the sockets connected
/// to the tunnel system. Removes inactive and dead connections
pub async fn tunnel_keep_alive(service: Arc<TunnelService>) {
    // Create the interval to track keep alive pings
    let keep_alive_start = Instant::now() + KEEP_ALIVE_DELAY;
    let mut keep_alive_interval = interval_at(keep_alive_start, KEEP_ALIVE_DELAY);
    let service = service.as_ref();

    keep_alive_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        // Wait for the next keep-alive tick
        keep_alive_interval.tick().await;

        // Remove any expired/closed tunnels
        {
            let now = Instant::now();

            service
                .mappings
                .write()
                .remove_dead_tunnels(now, KEEP_ALIVE_TIMEOUT);
        }

        // Send out keep-alive messages for any tunnels that aren't expired
        service
            .mappings
            .read()
            .tunnel_data()
            .for_each(|(tunnel_id, data)| {
                match &data.handle {
                    TunnelHandle::Udp(target_address) => {
                        let buffer = pocket_relay_udp_tunnel::serialize_message(
                            *tunnel_id,
                            &pocket_relay_udp_tunnel::TunnelMessage::KeepAlive,
                        );

                        // Send keep alive message
                        _ = service.udp_tx.send(UdpTunnelMessage {
                            buffer,
                            target_address: *target_address,
                        });
                    }
                    TunnelHandle::Http(tunnel_handle) => {
                        // Write a keep alive message
                        _ = tunnel_handle.tx.send(HttpTunnelMessage {
                            index: 255,
                            message: Bytes::new(),
                        });
                    }
                }
            });
    }
}
