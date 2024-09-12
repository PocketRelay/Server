//! Server side portion of the tunneling implementation
//!
//! Details can be found on the GitHub issue: https://github.com/PocketRelay/Server/issues/64

use self::codec::{TunnelCodec, TunnelMessage};
use crate::utils::{hashing::IntHashMap, types::GameID};
use bytes::Bytes;
use futures_util::{Sink, Stream};
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use parking_lot::RwLock;
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    task::{ready, Context, Poll},
    time::Duration,
};
use tokio::{
    sync::mpsc,
    time::{interval_at, Instant, Interval, MissedTickBehavior},
};
use tokio_util::codec::Framed;

use super::sessions::AssociationId;

/// The port bound on clients representing the host player within the socket pool
pub const TUNNEL_HOST_LOCAL_PORT: u16 = 42132;

/// ID for a tunnel
type TunnelId = u32;
/// Index into a pool of tunnels
type PoolIndex = u8;
/// ID of a pool
type PoolId = GameID;

#[derive(Default)]
pub struct TunnelService {
    /// Stores the next available tunnel ID
    next_tunnel_id: AtomicU32,
    /// Underlying tunnel mappings
    mappings: RwLock<TunnelMappings>,
}

pub struct TunnelData {
    /// Association ID for the tunnel
    association: AssociationId,
    /// Handle for the tunnel
    handle: TunnelHandle,
}

/// Stores mappings between various tunnel objects
#[derive(Default)]
struct TunnelMappings {
    /// Mapping from [TunnelId]s to the actual [TunnelHandle] for communicating
    /// with the tunnel
    id_to_tunnel: IntHashMap<TunnelId, TunnelData>,

    /// Mapping from [AssociationId] (Client association) to [TunnelHandle]
    association_to_tunnel: HashMap<AssociationId, TunnelId>,

    /// Mapping associating a [TunnelId] with a [PoolIndex] within a [PoolId]
    /// that it is apart of
    tunnel_to_index: IntHashMap<TunnelId, PoolKey>,
    /// Inverse mapping of `tunnel_to_index` for finding the handle
    /// associated to a specific pool and slot
    index_to_tunnel: IntHashMap<PoolKey, TunnelId>,
}

/// Represents a key that is created from a [PoolId] and [PoolIndex] combined
/// into a single value.
///
/// This allows it to be used as a key in the [IntHashMap]
#[derive(Hash, PartialEq, Eq, Clone, Copy)]
struct PoolKey(u64);

impl PoolKey {
    /// Creates a new pool key from its components
    const fn new(pool_id: PoolId, pool_index: PoolIndex) -> Self {
        Self(((pool_id as u64) << 32) | pool_index as u64)
    }

    /// Converts the key into its underlying parts
    const fn parts(&self) -> (PoolId, PoolIndex) {
        ((self.0 >> 32) as PoolId, self.0 as PoolIndex)
    }
}

impl TunnelMappings {
    // Inserts a new tunnel into the mappings
    fn insert_tunnel(&mut self, tunnel_id: TunnelId, tunnel: TunnelData) {
        // Insert the tunnel into the mappings
        self.id_to_tunnel.insert(tunnel_id, tunnel);
    }

    /// Associates the provided `association` to the provided `tunnel`
    ///
    /// Creates a mapping for the [AssociationId] to [TunnelHandle] along
    /// with [TunnelHandle] to [AssociationId]
    fn associate_tunnel(&mut self, association: AssociationId, tunnel_id: TunnelId) {
        // Create the IP relationship
        self.association_to_tunnel.insert(association, tunnel_id);
    }

    /// Attempts to associate the tunnel from `address` to the provided
    /// `pool_id` and `pool_index` if there is a tunnel connected to
    /// `address`
    fn associate_pool(
        &mut self,
        association: AssociationId,
        pool_id: PoolId,
        pool_index: PoolIndex,
    ) {
        let tunnel_id = match self.association_to_tunnel.get(&association) {
            Some(value) => *value,
            None => return,
        };

        let key = PoolKey::new(pool_id, pool_index);

        self.tunnel_to_index.insert(tunnel_id, key);
        self.index_to_tunnel.insert(key, tunnel_id);
    }

    /// Uses the lookup maps to find the [TunnelHandle] of another tunnel within the same
    /// pool as `tunnel_id` at the provided `pool_index`.
    ///
    /// Returns both the [TunnelHandle] at `pool_index` and the [PoolIndex] of the
    /// provided `tunnel_id`
    fn get_tunnel_route(
        &self,
        tunnel_id: TunnelId,
        pool_index: PoolIndex,
    ) -> Option<(TunnelHandle, PoolIndex)> {
        let (game_id, self_index) = self.tunnel_to_index.get(&tunnel_id)?.parts();
        let other_tunnel = self
            .index_to_tunnel
            .get(&PoolKey::new(game_id, pool_index))?;
        let tunnel = self.id_to_tunnel.get(other_tunnel)?;

        Some((tunnel.handle.clone(), self_index))
    }

    /// Removes the association between the `tunnel_id` and any games and
    /// removes the tunnel itself.
    ///
    /// Used when a tunnel disconnects to remove any associations
    /// related to the tunnel
    fn dissociate_tunnel(&mut self, tunnel_id: TunnelId) {
        // Remove tunnel itself
        let tunnel_data = self.id_to_tunnel.remove(&tunnel_id);

        if let Some(tunnel_data) = tunnel_data {
            self.association_to_tunnel.remove(&tunnel_data.association);
        }

        // Remove the slot association
        if let Some(key) = self.tunnel_to_index.remove(&tunnel_id) {
            // Remove the inverse relationship
            self.index_to_tunnel.remove(&key);
        }
    }

    /// Removes the association between a [PoolKey] and a [TunnelId] if
    /// one is present
    fn dissociate_pool(&mut self, pool_id: PoolId, pool_index: PoolIndex) {
        if let Some(tunnel_id) = self
            .index_to_tunnel
            .remove(&PoolKey::new(pool_id, pool_index))
        {
            self.tunnel_to_index.remove(&tunnel_id);
        }
    }
}

impl TunnelService {
    /// Wrapper around [`TunnelMappings::associate_tunnel`] that holds the service
    /// write lock before operating
    #[inline]
    pub fn associate_tunnel(&self, association: AssociationId, tunnel_id: TunnelId) {
        self.mappings
            .write()
            .associate_tunnel(association, tunnel_id)
    }

    /// Wrapper around [`TunnelMappings::associate_pool`] that holds the service
    /// write lock before operating
    #[inline]
    pub fn associate_pool(
        &self,
        association: AssociationId,
        pool_id: PoolId,
        pool_index: PoolIndex,
    ) {
        self.mappings
            .write()
            .associate_pool(association, pool_id, pool_index)
    }

    /// Wrapper around [`TunnelMappings::get_tunnel_route`] that holds the service
    /// read lock before operating
    #[inline]
    pub fn get_tunnel_route(
        &self,
        tunnel_id: TunnelId,
        pool_index: PoolIndex,
    ) -> Option<(TunnelHandle, PoolIndex)> {
        self.mappings.read().get_tunnel_route(tunnel_id, pool_index)
    }

    /// Wrapper around [`TunnelMappings::dissociate_tunnel`] that holds the service
    /// write lock before operating
    #[inline]
    pub fn dissociate_tunnel(&self, tunnel_id: TunnelId) {
        self.mappings.write().dissociate_tunnel(tunnel_id);
    }

    /// Wrapper around [`TunnelMappings::dissociate_pool`] that holds the service
    /// write lock before operating
    #[inline]
    pub fn dissociate_pool(&self, pool_id: PoolId, pool_index: PoolIndex) {
        self.mappings.write().dissociate_pool(pool_id, pool_index);
    }
}

/// Handle for sending messages to a tunnel
#[derive(Clone)]
pub struct TunnelHandle {
    /// The sender for sending messages to the tunnel
    tx: mpsc::UnboundedSender<TunnelMessage>,
}

/// Tunnel connection to a client
pub struct Tunnel {
    /// ID for this tunnel
    id: TunnelId,
    /// The IO tunnel used to send information to the host and receive
    /// response
    io: Framed<TokioIo<Upgraded>, TunnelCodec>,
    /// Receiver for messages that should be written to the tunnel
    rx: mpsc::UnboundedReceiver<TunnelMessage>,
    /// Future state for writing to the `io`
    write_state: TunnelWriteState,
    /// The service access
    service: Arc<TunnelService>,
    /// Interval for sending keep alive messages
    keep_alive_interval: Interval,
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        // Remove the tunnel from the service
        self.service.dissociate_tunnel(self.id);
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
    /// The tunnel has stopped and should not continue
    Stop,
}

/// Holds the state for the current reading progress for a [`Tunnel`]
enum TunnelReadState {
    /// Continue reading
    Continue,
    /// The tunnel has stopped and should not continue
    Stop,
}

impl Tunnel {
    // Send keep-alive pings every 10s
    const KEEP_ALIVE_DELAY: Duration = Duration::from_secs(10);

    /// Starts a new tunnel on `io` using the tunnel `service`
    ///
    /// ## Arguments
    /// * `service`     - The service to add the tunnel to
    /// * `association` - The client association ID for this tunnel
    /// * `io`          - The underlying tunnel IO
    pub fn start(
        service: Arc<TunnelService>,
        association: AssociationId,
        io: Upgraded,
    ) -> TunnelId {
        let (tx, rx) = mpsc::unbounded_channel();

        // Wrap the `io` with the [`TunnelCodec`] for framing
        let io = Framed::new(TokioIo::new(io), TunnelCodec::default());

        // Acquire the tunnel ID
        let id = service.next_tunnel_id.fetch_add(1, Ordering::AcqRel);

        // Store the tunnel mapping
        service.mappings.write().insert_tunnel(
            id,
            TunnelData {
                association,
                handle: TunnelHandle { tx },
            },
        );

        // Create the interval to track keep alive pings
        let keep_alive_start = Instant::now() + Self::KEEP_ALIVE_DELAY;
        let mut keep_alive_interval = interval_at(keep_alive_start, Self::KEEP_ALIVE_DELAY);

        keep_alive_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        // Spawn the tunnel task
        tokio::spawn(Tunnel {
            service,
            id,
            io,
            rx,
            write_state: Default::default(),
            keep_alive_interval,
        });

        id
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

        // Ping messages can be ignored
        if message.index == 255 {
            return Poll::Ready(TunnelReadState::Continue);
        }

        // Get the path through the tunnel
        let (target_handle, index) = match self.service.get_tunnel_route(self.id, message.index) {
            Some(value) => value,
            // Don't have a tunnel to send the message through
            None => return Poll::Ready(TunnelReadState::Continue),
        };

        // Update the message target index to be from the correct index
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

        // Write a ping message at the interval if we aren't already sending a message
        if this.keep_alive_interval.poll_tick(cx).is_ready() {
            if let TunnelWriteState::Recv = this.write_state {
                // Move to a writing state
                this.write_state = TunnelWriteState::Write(Some(TunnelMessage {
                    index: 255,
                    message: Bytes::new(),
                }));

                // Poll the writer with the new message
                if let Poll::Ready(next_state) = this.poll_write_state(cx) {
                    this.write_state = next_state;

                    // Tunnel has stopped
                    if let TunnelWriteState::Stop = this.write_state {
                        return Poll::Ready(());
                    }
                }
            }
        }

        Poll::Pending
    }
}

mod codec {
    //! This modules contains the codec and message structures for [TunnelMessage]s
    //!
    //! # Tunnel Messages
    //!
    //! Tunnel message frames are as follows:
    //!
    //! ```norun
    //!  0                   1                   2                      
    //!  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3
    //! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    //! |     Index     |            Length             |
    //! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    //! |                                               :
    //! :                    Payload                    :
    //! :                                               |
    //! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    //! ```
    //!
    //! Tunnel message frames contain the following fields:
    //!
    //! Index: 8-bits. Determines the destination of the message within the current pool.
    //!
    //! Length: 16-bits. Determines the size in bytes of the payload that follows
    //!
    //! Payload: Variable length. The message bytes payload of `Length`
    //!
    //!
    //! ## Keep alive
    //!
    //! The server will send keep-alive messages, these are in the same
    //! format as the packet above. However, the index will always be 255
    //! and the payload will be empty.

    use bytes::{Buf, BufMut, Bytes};
    use tokio_util::codec::{Decoder, Encoder};

    /// Header portion of a [TunnelMessage] that contains the
    /// index of the message and the length of the expected payload
    struct TunnelMessageHeader {
        /// Socket index to use
        index: u8,
        /// The length of the tunnel message bytes
        length: u16,
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
        /// Stores the current message header while its waiting
        /// for the full payload to become available
        partial: Option<TunnelMessageHeader>,
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
                    let length = src.get_u16();

                    self.partial.insert(TunnelMessageHeader { index, length })
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
            dst.put_u16(item.message.len() as u16);
            dst.extend_from_slice(&item.message);
            Ok(())
        }
    }
}
