//! Server side portion of the tunneling implementation
//!
//! Details can be found on the GitHub issue: https://github.com/PocketRelay/Server/issues/64

use self::codec::{TunnelCodec, TunnelMessage};
use crate::utils::{hashing::IntHashMap, types::GameID};
use futures_util::{Sink, Stream};
use hashbrown::HashMap;
use hyper::upgrade::Upgraded;
use parking_lot::Mutex;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    task::{ready, Context, Poll},
};
use tokio::sync::mpsc;
use tokio_util::codec::Framed;

/// The port bound on clients representing the host player within the socket poool
pub const TUNNEL_HOST_LOCAL_PORT: u16 = 42132;

/// ID for a tunnel
type TunnelId = u32;
/// Index into a pool of tunnels
type PoolIndex = u8;
/// Int created from an IPv4 address bytes
type Ipv4Int = u32;

#[derive(Default)]
pub struct TunnelService {
    /// Stores the next available tunnel ID
    next_tunnel_id: AtomicU32,
    /// Mapping between host addreses and access to their tunnel
    tunnels: Mutex<IntHashMap<Ipv4Int, TunnelHandle>>,
    /// Tunnel pooling allocated for games
    pools: Mutex<IntHashMap<GameID, TunnelPool>>,
    /// Mapping for which game a tunnel is connected to
    mapping: Mutex<TunnelMapping>,
}

/// Stores mappings between tunnels and game slots and
/// the inverse
#[derive(Default)]
struct TunnelMapping {
    /// Mapping from tunnel IDs to game slots
    tunnel_to_slot: IntHashMap<TunnelId, (GameID, PoolIndex)>,
    /// Mapping from game slots to tunnel IDs
    slot_to_tunnel: HashMap<(GameID, PoolIndex), TunnelId>,
}

impl TunnelMapping {
    /// Inserts mappings for the provided `tunnel_id`, `game_id` and `pool_index`
    pub fn insert(&mut self, tunnel_id: TunnelId, game_id: GameID, pool_index: PoolIndex) {
        self.tunnel_to_slot.insert(tunnel_id, (game_id, pool_index));
        self.slot_to_tunnel.insert((game_id, pool_index), tunnel_id);
    }

    /// Removes a mapping using a `pool_index` within a `game_id`
    pub fn remove_by_slot(&mut self, game_id: GameID, pool_index: PoolIndex) -> Option<TunnelId> {
        if let Some(tunnel_id) = self.slot_to_tunnel.remove(&(game_id, pool_index)) {
            self.tunnel_to_slot.remove(&tunnel_id);

            Some(tunnel_id)
        } else {
            None
        }
    }

    /// Removes a mapping using the `tunnel_id`
    pub fn remove_by_tunnel(&mut self, tunnel_id: TunnelId) -> Option<(GameID, PoolIndex)> {
        if let Some(key) = self.tunnel_to_slot.remove(&tunnel_id) {
            self.slot_to_tunnel.remove(&key);
            Some(key)
        } else {
            None
        }
    }

    /// Gets a tunnel by its `tunnel_id`
    pub fn get_by_tunnel(&self, tunnel_id: TunnelId) -> Option<(GameID, PoolIndex)> {
        self.tunnel_to_slot
            // Find the mapping for the tunnel
            .get(&tunnel_id)
            // Take a copy of the values if present
            .copied()
    }
}

impl TunnelService {
    /// Removes a game from the pool using its [`GameID`]
    #[inline]
    pub fn remove_pool(&self, pool: GameID) {
        self.pools.lock().remove(&pool);
    }

    /// Finds the [`GameID`] and [`PoolIndex`] that are associated to
    /// the provided [`TunnelId`] if one is present
    #[inline]
    pub fn get_by_tunnel(&self, tunnel_id: TunnelId) -> Option<(GameID, PoolIndex)> {
        self.mapping.lock().get_by_tunnel(tunnel_id)
    }

    /// Removes a tunnel mapping and its handle from the game pool using the
    /// [`GameID`] and the [`PoolIndex`] for the mapping
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
    /// Removes a tunnel mapping and its handle from the game pool using the
    /// [`TunnelId`] for the mapping
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

    /// Gets the [`TunnelHandle`] for the [`PoolIndex`] within the pool for [`GameID`]
    /// if there is a [`TunnelHandle`] present at the provided index
    pub fn get_pool_handle(&self, pool: GameID, index: PoolIndex) -> Option<TunnelHandle> {
        // Access the pools map
        let pools = &*self.pools.lock();
        // Ge the pool for the `pool`
        let pool = pools.get(&pool)?;
        // Get the handle
        pool.handles.get(index as usize)?.clone()
    }

    /// Gets the tunnel for the provided IP address if one is present
    pub fn get_tunnel(&self, addr: Ipv4Int) -> Option<TunnelHandle> {
        self.tunnels.lock().get(&addr).cloned()
    }

    /// Sets the [`TunnelHandle`] for a specific [`Ipv4Addr`] updates
    /// existing tunnel mappings if they are present
    pub fn set_tunnel(&self, addr: Ipv4Int, tunnel: TunnelHandle) {
        self.tunnels.lock().insert(addr, tunnel);
    }

    /// Associates the provided `handle` with the `index` inside the provided
    /// `game_id` poool
    ///
    /// Creates a mapping and stores the pool handle
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

/// Represents a pool of tunnel connections
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
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        // Remove the tunnel from the service
        self.service.remove_by_tunnel(self.id);
    }
}

/// Holds the state for the current writing progress for a [`Tunnel`]
#[derive(Default)]
enum TunnelWriteState {
    /// Waiting for a message to come through the [`Tunnel::rx`]
    #[default]
    Recv,
    /// Waiting for the [`Tunnel::io`] to be writable, then writing the
    /// contained [`TunnelMessage`]
    Write(Option<TunnelMessage>),
    /// Poll flushing the bytes written to [`Tunnel::io`]
    Flush,
    /// The tunnnel has stopped and should not continue
    Stop,
}

/// Holds the state for the current reading progress for a [`Tunnel`]
enum TunnelReadState {
    /// Continue reading
    Continue,
    /// The tunnnel has stopped and should not continue
    Stop,
}

impl Tunnel {
    /// Starts a new tunnel on `io` using the tunnel `service`
    ///
    /// ## Arguments
    /// * `service` - The server to add the tunnel to
    /// * `io`      - The underlying tunnel IO
    pub fn start(service: Arc<TunnelService>, io: Upgraded) -> TunnelHandle {
        let (tx, rx) = mpsc::unbounded_channel();

        // Wrap the `io` with the [`TunnelCodec`] for framing
        let io = Framed::new(io, TunnelCodec::default());

        // Aquire the tunnel ID
        let id = service.next_tunnel_id.fetch_add(1, Ordering::AcqRel);

        tokio::spawn(Tunnel {
            service,
            id,
            io,
            rx,
            write_state: Default::default(),
        });

        TunnelHandle { id, tx }
    }

    /// Polls accepting messages from [`Tunnel::rx`] then writing them to [`Tunnel::io`] and
    /// flushing the underlying stream. Provides the next [`TunnelWriteState`]
    /// when [`Poll::Ready`] is returned
    ///
    /// Should be repeatedly called until it no-longer returns [`Poll::Ready`]
    fn poll_write_state(&mut self, cx: &mut Context<'_>) -> Poll<TunnelWriteState> {
        Poll::Ready(match &mut self.write_state {
            TunnelWriteState::Recv => {
                // Try receive a packet from the write channel
                let result = ready!(Pin::new(&mut self.rx).poll_recv(cx));

                if let Some(message) = result {
                    TunnelWriteState::Write(Some(message))
                } else {
                    // All writers have closed, tunnel must be closed (Future end)
                    TunnelWriteState::Stop
                }
            }
            TunnelWriteState::Write(message) => {
                // Wait until the `io` is ready
                if ready!(Pin::new(&mut self.io).poll_ready(cx)).is_ok() {
                    let message = message
                        .take()
                        .expect("Unexpected write state without message");

                    // Write the packet to the buffer
                    Pin::new(&mut self.io)
                        .start_send(message)
                        // Packet encoder impl shouldn't produce errors
                        .expect("Message encoder errored");

                    TunnelWriteState::Flush
                } else {
                    // Failed to ready, tunnel must be closed
                    TunnelWriteState::Stop
                }
            }
            TunnelWriteState::Flush => {
                // Poll flushing `io`
                if ready!(Pin::new(&mut self.io).poll_flush(cx)).is_ok() {
                    TunnelWriteState::Recv
                } else {
                    // Failed to flush, tunnel must be closed
                    TunnelWriteState::Stop
                }
            }

            // Tunnel should *NOT* be polled if its already stopped
            TunnelWriteState::Stop => panic!("Tunnel polled after already stopped"),
        })
    }

    /// Polls reading messages from [`Tunnel::io`] and sending them to the correct
    /// handle within the [`Tunnel::pool`]. Provides the next [`TunnelReadState`]
    /// when [`Poll::Ready`] is returned
    ///
    /// Should be repeatedly called until it no-longer returns [`Poll::Ready`]
    fn poll_read_state(&mut self, cx: &mut Context<'_>) -> Poll<TunnelReadState> {
        // Try receive a message from the `io`
        let Some(Ok(mut message)) = ready!(Pin::new(&mut self.io).poll_next(cx)) else {
            // Cannot read next message stop the tunnel
            return Poll::Ready(TunnelReadState::Stop);
        };

        // Get the tunnel sender details
        let (game_id, index) = match self.service.get_by_tunnel(self.id) {
            Some(value) => value,
            // Don't have a tunnel to send the message through
            None => return Poll::Ready(TunnelReadState::Continue),
        };

        // Get the handle the message is for
        let target_handle = match self.service.get_pool_handle(game_id, message.index) {
            Some(value) => value,
            // Don't have an associated handle to send the message to
            None => return Poll::Ready(TunnelReadState::Continue),
        };

        // Update the message source index using the sender
        message.index = index;

        // Send the message to the tunnel
        _ = target_handle.tx.send(message);

        Poll::Ready(TunnelReadState::Continue)
    }
}

impl Future for Tunnel {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        // Poll the write half
        while let Poll::Ready(next_state) = this.poll_write_state(cx) {
            this.write_state = next_state;

            // Tunnel has stopped
            if let TunnelWriteState::Stop = this.write_state {
                return Poll::Ready(());
            }
        }

        // Poll the read half
        while let Poll::Ready(next_state) = this.poll_read_state(cx) {
            // Tunnel has stopped
            if let TunnelReadState::Stop = next_state {
                return Poll::Ready(());
            }
        }

        Poll::Pending
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
