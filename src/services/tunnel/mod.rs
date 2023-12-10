//! Clients connect through this service in order to form a connection
//! with a host player without the possible NAT restrictions that may
//! occur on stricter NATs
//!
//!
//!
//!
//! Client(s) -- Sends packets to -> Local host socket
//!
//! Local host socket -- Sends packet with index 0 --> Server
//!
//! Server -- Forwards packets to --> Host local pool
//!
//! Host local pool -- Sends packet pretending to be the other client --> Host
//!
//! Host -- Sends reply to --> Host local pool
//!
//! Host local pool -- Sends reply with index --> Server
//!
//! Server -- Forwards packets to index --> Client
//!

use self::codec::{TunnelCodec, TunnelMessage};
use crate::utils::hashing::IntHashMap;
use crate::utils::types::GameID;
use futures_util::{Sink, Stream};
use hashbrown::HashMap;
use hyper::upgrade::Upgraded;
use parking_lot::Mutex;
use std::future::Future;
use std::{
    net::Ipv4Addr,
    pin::Pin,
    sync::{atomic::AtomicU32, Arc},
    task::{ready, Context, Poll},
};
use tokio::sync::mpsc;
use tokio_util::codec::Framed;

static TUNNEL_ID: AtomicU32 = AtomicU32::new(1);

// ID for a tunnel
type TunnelId = u32;
// Index into a pool of tunnels
type PoolIndex = u8;

#[derive(Default)]
pub struct TunnelService {
    /// Mapping between host addreses and access to their tunnel
    tunnels: Mutex<HashMap<Ipv4Addr, TunnelHandle>>,
    /// Tunnel pooling allocated for games
    pools: Mutex<IntHashMap<GameID, TunnelPool>>,
    /// Mapping for which game a tunnel is connected to
    mapping: Mutex<TunnelMapping>,
}

/// Stores mappings between tunnels and game slots and
/// the inverse
#[derive(Default)]
pub struct TunnelMapping {
    /// Mapping from tunnel IDs to game slots
    tunnel_to_slot: IntHashMap<TunnelId, (GameID, PoolIndex)>,
    /// Mapping from game slots to tunnel IDs
    slot_to_tunnel: HashMap<(GameID, PoolIndex), TunnelId>,
}

impl TunnelMapping {
    pub fn insert(&mut self, tunnel_id: TunnelId, game_id: GameID, pool_index: PoolIndex) {
        self.tunnel_to_slot.insert(tunnel_id, (game_id, pool_index));
        self.slot_to_tunnel.insert((game_id, pool_index), tunnel_id);
    }

    pub fn remove_by_slot(&mut self, game_id: GameID, pool_index: PoolIndex) -> Option<TunnelId> {
        if let Some(tunnel_id) = self.slot_to_tunnel.remove(&(game_id, pool_index)) {
            self.tunnel_to_slot.remove(&tunnel_id);

            Some(tunnel_id)
        } else {
            None
        }
    }

    pub fn remove_by_tunnel(&mut self, tunnel_id: TunnelId) -> Option<(GameID, PoolIndex)> {
        if let Some(key) = self.tunnel_to_slot.remove(&tunnel_id) {
            self.slot_to_tunnel.remove(&key);
            Some(key)
        } else {
            None
        }
    }

    pub fn get_by_tunnel(&self, tunnel_id: TunnelId) -> Option<(GameID, PoolIndex)> {
        self.tunnel_to_slot
            // Find the mapping for the tunnel
            .get(&tunnel_id)
            // Take a copy of the values if present
            .copied()
    }
}

impl TunnelService {
    // Removes a game from the pool
    #[inline]
    pub fn remove_pool(&self, pool: GameID) {
        self.pools.lock().remove(&pool);
    }

    #[inline]
    pub fn get_by_tunnel(&self, tunnel_id: TunnelId) -> Option<(GameID, PoolIndex)> {
        self.mapping.lock().get_by_tunnel(tunnel_id)
    }

    pub fn remove_by_slot(&self, game_id: GameID, pool_index: PoolIndex) {
        self.mapping.lock().remove_by_slot(game_id, pool_index);

        // Remove the handle from its associated pool
        let pools = &mut *self.pools.lock();
        if let Some(pool) = pools.get_mut(&game_id) {
            if let Some(handle) = pool.handles.get_mut(pool_index as usize) {
                *handle = None;
            }
        }
    }

    pub fn remove_by_tunnel(&self, tunnel_id: TunnelId) {
        if let Some((game_id, pool_index)) = self.mapping.lock().remove_by_tunnel(tunnel_id) {
            // Remove the handle from its associated pool
            let pools = &mut *self.pools.lock();
            if let Some(pool) = pools.get_mut(&game_id) {
                if let Some(handle) = pool.handles.get_mut(pool_index as usize) {
                    *handle = None;
                }
            }
        }
    }

    pub fn get_pool_handle(&self, pool: GameID, index: PoolIndex) -> Option<TunnelHandle> {
        // Access the pools map
        let pools = &*self.pools.lock();
        // Ge the pool for the `pool`
        let pool = pools.get(&pool)?;
        // Get the handle
        pool.handles.get(index as usize)?.clone()
    }

    /// Gets the tunnel for the provided IP address if one is present
    pub fn get_tunnel(&self, addr: Ipv4Addr) -> Option<TunnelHandle> {
        let tunnels = &*self.tunnels.lock();
        tunnels.get(&addr).cloned()
    }

    pub fn set_tunnel(&self, addr: Ipv4Addr, tunnel: TunnelHandle) {
        let tunnels = &mut *self.tunnels.lock();
        tunnels.insert(addr, tunnel);
    }

    /// Sets the handle at the provided index within a pool to the provided handle
    pub fn set_pool_handle(&self, game_id: GameID, index: usize, handle: TunnelHandle) {
        // Assocate the handle with the game
        {
            self.mapping
                .lock()
                // Map the handle to its game
                .insert(handle.id, game_id, index as PoolIndex);
        }

        let pools = &mut *self.pools.lock();

        // Get the existing pool or insert a new one
        let pool = pools.entry(game_id).or_default();

        if let Some(pool_handle) = pool.handles.get_mut(index) {
            *pool_handle = Some(handle);
        }
    }
}

/// Represents a pool of tunnel ocnnections
#[derive(Default)]
struct TunnelPool {
    /// Collection of client handles
    handles: [Option<TunnelHandle>; 4],
}

/// Handle for sending messages to a tunnel
#[derive(Clone)]
pub struct TunnelHandle {
    /// The ID of the tunnel
    id: TunnelId,
    /// The sender for sending messages to the tunnel
    tx: mpsc::UnboundedSender<TunnelMessage>,
}

/// Represents a connection to a client tunnel
pub struct Tunnel {
    /// ID for this tunnel
    id: TunnelId,
    /// The IO tunnel used to send information to the host and recieve
    /// respones
    io: Framed<Upgraded, TunnelCodec>,
    /// Reciever for messages that should be written to the tunnel
    rx: mpsc::UnboundedReceiver<TunnelMessage>,
    /// The service access
    service: Arc<TunnelService>,
    /// Future state for writing to the `io`
    write_state: TunnelWriteState,
    /// Whether the future has been stopped
    stop: bool,
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        // Remove the tunnel from the service
        self.service.remove_by_tunnel(self.id);
    }
}

enum TunnelWriteState {
    /// Recieve the message to write
    Recv,
    /// Wait for the stream to be writable
    Write {
        // The message to write
        message: Option<TunnelMessage>,
    },
    // Poll flushing the tunnel
    Flush,
}

impl Tunnel {
    pub fn start(service: Arc<TunnelService>, io: Upgraded) -> TunnelHandle {
        let (tx, rx) = mpsc::unbounded_channel();
        let io = Framed::new(io, TunnelCodec::default());
        let id = TUNNEL_ID.fetch_add(1, std::sync::atomic::Ordering::AcqRel);

        tokio::spawn(Tunnel {
            service,
            id,
            io,
            rx,
            write_state: TunnelWriteState::Recv,
            stop: false,
        });

        TunnelHandle { id, tx }
    }

    fn poll_write_state(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        match &mut self.write_state {
            TunnelWriteState::Recv => {
                // Try receive a packet from the write channel
                let result = ready!(Pin::new(&mut self.rx).poll_recv(cx));

                if let Some(message) = result {
                    self.write_state = TunnelWriteState::Write {
                        message: Some(message),
                    };
                } else {
                    // All writers have closed, session must be closed (Future end)
                    self.stop = true;
                }
            }
            TunnelWriteState::Write { message } => {
                // Wait until the inner is ready
                if ready!(Pin::new(&mut self.io).poll_ready(cx)).is_ok() {
                    let message = message
                        .take()
                        .expect("Unexpected write state without message");

                    // Write the packet to the buffer
                    Pin::new(&mut self.io)
                        .start_send(message)
                        // Packet encoder impl shouldn't produce errors
                        .expect("Message encoder errored");

                    self.write_state = TunnelWriteState::Flush;
                } else {
                    // Failed to ready, session must be closed
                    self.stop = true;
                }
            }
            TunnelWriteState::Flush => {
                // Wait until the flush is complete
                if ready!(Pin::new(&mut self.io).poll_flush(cx)).is_ok() {
                    self.write_state = TunnelWriteState::Recv;
                } else {
                    // Failed to flush, session must be closed
                    self.stop = true
                }
            }
        }

        Poll::Ready(())
    }

    fn poll_read_state(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        // Try receive a message from the `io`
        let result = ready!(Pin::new(&mut self.io).poll_next(cx));

        if let Some(Ok(mut message)) = result {
            // Get the tunnel sender details
            let (game_id, index) = match self.service.get_by_tunnel(self.id) {
                Some(value) => value,
                None => return Poll::Ready(()),
            };

            // Get the handle the message is for
            let target_handle = match self.service.get_pool_handle(game_id, message.index) {
                Some(value) => value,
                None => return Poll::Ready(()),
            };

            // Update the message source index using the sender
            message.index = index;

            // Send the message to the tunnel
            _ = target_handle.tx.send(message);
        } else {
            // Reader has closed or reading encountered an error (Either way stop reading)
            self.stop = true;
        }

        Poll::Ready(())
    }
}

impl Future for Tunnel {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        while this.poll_write_state(cx).is_ready() {}
        while this.poll_read_state(cx).is_ready() {}

        if this.stop {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

/// Encoding an decoding logic for tunnel packet messages
mod codec {
    use bytes::{Buf, BufMut, Bytes};
    use tokio_util::codec::{Decoder, Encoder};

    /// Partially decoded tunnnel message
    pub struct TunnelMessagePartial {
        /// Socket index to use
        pub index: u8,
        /// The length of the tunnel message bytes
        pub length: u32,
    }

    /// Message sent through the tunnel
    pub struct TunnelMessage {
        /// Socket index to use
        pub index: u8,
        /// The message contents
        pub message: Bytes,
    }

    /// Codec for encoding and decoding tunnel messages
    #[derive(Default)]
    pub struct TunnelCodec {
        /// Stores a partially decoded frame if one is present
        partial: Option<TunnelMessagePartial>,
    }

    impl Decoder for TunnelCodec {
        type Item = TunnelMessage;
        type Error = std::io::Error;

        fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
            let partial = match self.partial.as_mut() {
                Some(value) => value,
                None => {
                    // Not enough room for a partial frame
                    if src.len() < 5 {
                        return Ok(None);
                    }
                    let index = src.get_u8();
                    let length = src.get_u32();

                    self.partial.insert(TunnelMessagePartial { index, length })
                }
            };
            // Not enough data for the partial frame
            if src.len() < partial.length as usize {
                return Ok(None);
            }

            let partial = self.partial.take().expect("Partial frame missing");
            let bytes = src.split_to(partial.length as usize);

            Ok(Some(TunnelMessage {
                index: partial.index,
                message: bytes.freeze(),
            }))
        }
    }

    impl Encoder<TunnelMessage> for TunnelCodec {
        type Error = std::io::Error;

        fn encode(
            &mut self,
            item: TunnelMessage,
            dst: &mut bytes::BytesMut,
        ) -> Result<(), Self::Error> {
            dst.put_u8(item.index);
            dst.put_u32(item.message.len() as u32);
            dst.extend_from_slice(&item.message);
            Ok(())
        }
    }
}
