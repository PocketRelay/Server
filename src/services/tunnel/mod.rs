use bytes::Bytes;
use http_tunnel::HttpTunnelMessage;
use mappings::{TunnelHandle, TunnelMappings};
use parking_lot::RwLock;
use tokio::sync::mpsc;
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
        self.mappings
            .write()
            .associate_pool(association, pool_id, pool_index);
    }

    pub fn dissociate_pool(&self, pool_id: PoolId, pool_index: PoolIndex) {
        self.mappings.write().dissociate_pool(pool_id, pool_index);
    }

    pub fn dissociate_tunnel_http(&self, tunnel_id: TunnelId) {
        self.mappings.write().dissociate_tunnel(tunnel_id);
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
        // Get the path through the tunnel
        let (target_handle, from_index) = {
            match self
                .mappings
                .read()
                .get_tunnel_route(from_tunnel_id, to_index)
            {
                Some(value) => value,
                // Don't have a tunnel to send the message through
                None => {
                    return;
                }
            }
        };

        // Forward message to target tunnel
        match target_handle {
            TunnelHandle::Udp(socket_addr) => {
                let message = match buffer {
                    TunnelBuffer::Owned(items) => items,
                    TunnelBuffer::Shared(bytes) => bytes.into(),
                };

                let buffer: Vec<u8> = pocket_relay_udp_tunnel::serialize_message(
                    from_tunnel_id,
                    &pocket_relay_udp_tunnel::TunnelMessage::Forward {
                        index: from_index,
                        message,
                    },
                );

                _ = self.udp_tx.send(UdpTunnelMessage {
                    buffer,
                    target_address: socket_addr,
                });
            }
            TunnelHandle::Http(tunnel_handle) => {
                let message = match buffer {
                    TunnelBuffer::Owned(items) => items.into(),
                    TunnelBuffer::Shared(bytes) => bytes,
                };

                _ = tunnel_handle.tx.send(HttpTunnelMessage {
                    index: from_index,
                    message,
                });
            }
        }
    }
}

pub enum TunnelBuffer {
    Owned(Vec<u8>),
    Shared(Bytes),
}
