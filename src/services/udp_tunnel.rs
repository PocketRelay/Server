use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};

use codec::{MessageHeader, MessageReader, MessageWriter, TunnelMessage};
use log::{debug, error};
use parking_lot::RwLock;
use tokio::{
    net::UdpSocket,
    time::{interval_at, Instant, MissedTickBehavior},
};
use uuid::Uuid;

use crate::utils::{hashing::IntHashMap, types::GameID};

use super::sessions::{AssociationId, Sessions};

/// The port bound on clients representing the host player within the socket pool
pub const _TUNNEL_HOST_LOCAL_PORT: u16 = 42132;

/// ID for a tunnel
type TunnelId = u32;
/// Index into a pool of tunnels
type PoolIndex = u8;
/// ID of a pool
type PoolId = GameID;

pub async fn create_tunnel_service(
    sessions: Arc<Sessions>,
    tunnel_addr: SocketAddr,
) -> Arc<TunnelServiceV2> {
    let socket = UdpSocket::bind(tunnel_addr).await.unwrap();
    let service = Arc::new(TunnelService::new(socket, sessions));

    debug!("started tunneling server {tunnel_addr}");

    // Spawn the task to handle accepting messages
    tokio::spawn(accept_messages(service.clone()));

    // Spawn task to keep connections alive
    tokio::spawn(keep_alive(service.clone()));

    service
}

/// Reads inbound messages from the tunnel service
pub async fn accept_messages(service: Arc<TunnelService>) {
    // Buffer to recv messages
    let mut buffer = [0; u16::MAX as usize];

    loop {
        let (size, addr) = match service.socket.recv_from(&mut buffer).await {
            Ok(value) => value,
            Err(err) => {
                error!("failed to recv message: {err}");
                continue;
            }
        };

        let buffer = &buffer[0..size];
        let mut reader = MessageReader::new(buffer);

        let header = match MessageHeader::read(&mut reader) {
            Ok(value) => value,
            Err(_err) => {
                error!("invalid message header");
                continue;
            }
        };

        let message = match TunnelMessage::read(&mut reader) {
            Ok(value) => value,
            Err(_err) => {
                error!("invalid message");
                continue;
            }
        };

        debug!("got message: {:?}", message);

        let tunnel_id = header.tunnel_id;

        // Handle the message through a background task
        let service = service.clone();
        tokio::spawn(async move {
            service.handle_message(tunnel_id, message, addr).await;
        });
    }
}

const KEEP_ALIVE_DELAY: Duration = Duration::from_secs(5);
const KEEP_ALIVE_TIMEOUT: Duration = Duration::from_secs(60);

pub async fn keep_alive(service: Arc<TunnelService>) {
    // Create the interval to track keep alive pings
    let keep_alive_start = Instant::now() + KEEP_ALIVE_DELAY;
    let mut keep_alive_interval = interval_at(keep_alive_start, KEEP_ALIVE_DELAY);

    keep_alive_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        keep_alive_interval.tick().await;

        let now = Instant::now();

        // Read the tunnels of all current tunnels
        let tunnels: Vec<(TunnelId, SocketAddr, Instant)> = {
            let mappings = &*service.mappings.read();

            mappings
                .id_to_tunnel
                .iter()
                .map(|(tunnel_id, value)| (*tunnel_id, value.addr, value.last_alive))
                .collect()
        };

        let mut expired_tunnels: Vec<TunnelId> = Vec::new();

        // Send out keep-alive messages for any tunnels that aren't expired

        for (tunnel_id, addr, last_alive) in tunnels {
            let last_alive = last_alive.duration_since(now);
            if last_alive > KEEP_ALIVE_TIMEOUT {
                expired_tunnels.push(tunnel_id);
                continue;
            }

            let mut buffer = MessageWriter::default();

            let header = MessageHeader {
                tunnel_id,
                version: 0,
            };
            let message = TunnelMessage::KeepAlive;

            // Write header and message
            header.write(&mut buffer);
            message.write(&mut buffer);

            // TODO: Parallel send
            service.socket.send_to(&buffer.buffer, addr).await.unwrap();
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

pub type TunnelServiceV2 = TunnelService;

pub struct TunnelService {
    socket: UdpSocket,
    next_tunnel_id: AtomicU32,
    mappings: RwLock<TunnelMappings>,
    sessions: Arc<Sessions>,
}

pub struct TunnelData {
    /// Association ID for the tunnel
    association: AssociationId,
    /// Address of the tunnel
    addr: SocketAddr,
    /// Last time a keep alive was obtained for the tunnel
    last_alive: Instant,
}

#[derive(Default)]
pub struct TunnelMappings {
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
    ) -> Option<(SocketAddr, PoolIndex)> {
        let (game_id, self_index) = self.tunnel_to_index.get(&tunnel_id)?.parts();
        let other_tunnel = self
            .index_to_tunnel
            .get(&PoolKey::new(game_id, pool_index))?;
        let tunnel = self.id_to_tunnel.get(other_tunnel)?;

        Some((tunnel.addr, self_index))
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
    pub fn new(socket: UdpSocket, sessions: Arc<Sessions>) -> Self {
        Self {
            socket,
            next_tunnel_id: AtomicU32::new(0),
            mappings: RwLock::new(TunnelMappings::default()),
            sessions,
        }
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
    ) -> Option<(SocketAddr, PoolIndex)> {
        self.mappings.read().get_tunnel_route(tunnel_id, pool_index)
    }

    /// Wrapper around [`TunnelMappings::dissociate_pool`] that holds the service
    /// write lock before operating
    #[inline]
    pub fn dissociate_pool(&self, pool_id: PoolId, pool_index: PoolIndex) {
        self.mappings.write().dissociate_pool(pool_id, pool_index);
    }

    /// Handles processing a message received through the tunnel
    async fn handle_message(&self, tunnel_id: u32, msg: TunnelMessage, addr: SocketAddr) {
        match msg {
            TunnelMessage::Initiate { association_token } => {
                let association = match self.sessions.verify_assoc_token(&association_token) {
                    Ok(value) => value,
                    Err(_err) => {
                        return;
                    }
                };

                // Acquire the tunnel ID
                let id = self.next_tunnel_id.fetch_add(1, Ordering::AcqRel);

                self.mappings
                    .write()
                    .associate_tunnel(association, tunnel_id);

                // Store the tunnel mapping
                self.mappings.write().insert_tunnel(
                    id,
                    TunnelData {
                        addr,
                        association,
                        last_alive: Instant::now(),
                    },
                );

                let mut buffer = MessageWriter::default();

                let header = MessageHeader {
                    tunnel_id,
                    version: 0,
                };
                let message = TunnelMessage::Initiated { tunnel_id: id };

                // Write header and message
                header.write(&mut buffer);
                message.write(&mut buffer);

                self.socket.send_to(&buffer.buffer, addr).await.unwrap();
            }
            TunnelMessage::Initiated { .. } => {
                // Server shouldn't be receiving this message... ignore it
            }
            TunnelMessage::Forward { index, message } => {
                // Get the path through the tunnel
                let (target_addr, index) = match self.get_tunnel_route(tunnel_id, index) {
                    Some(value) => value,
                    // Don't have a tunnel to send the message through
                    None => return,
                };

                let mut buffer = MessageWriter::default();

                let header = MessageHeader {
                    tunnel_id,
                    version: 0,
                };
                let message = TunnelMessage::Forward { index, message };

                // Write header and message
                header.write(&mut buffer);
                message.write(&mut buffer);

                self.socket
                    .send_to(&buffer.buffer, target_addr)
                    .await
                    .unwrap();
            }
            TunnelMessage::KeepAlive => {
                // Ack keep alive
            }
        }
    }
}

mod codec {
    use thiserror::Error;

    #[derive(Default)]
    pub struct MessageWriter {
        pub buffer: Vec<u8>,
    }

    impl MessageWriter {
        #[inline]
        pub fn write_u8(&mut self, value: u8) {
            self.buffer.push(value)
        }

        #[inline]
        pub fn write_bytes(&mut self, value: &[u8]) {
            self.buffer.extend_from_slice(value)
        }

        pub fn write_u32(&mut self, value: u32) {
            self.write_bytes(&value.to_be_bytes())
        }

        pub fn write_u16(&mut self, value: u16) {
            self.write_bytes(&value.to_be_bytes())
        }
    }

    pub struct MessageReader<'a> {
        buffer: &'a [u8],
        cursor: usize,
    }

    impl<'a> MessageReader<'a> {
        pub fn new(buffer: &'a [u8]) -> MessageReader<'a> {
            MessageReader { buffer, cursor: 0 }
        }

        #[inline]
        pub fn capacity(&self) -> usize {
            self.buffer.len()
        }

        pub fn len(&self) -> usize {
            self.capacity() - self.cursor
        }

        pub fn read_u8(&mut self) -> Result<u8, MessageError> {
            if self.len() < 1 {
                return Err(MessageError::Incomplete);
            }

            let value = self.buffer[self.cursor];
            self.cursor += 1;

            Ok(value)
        }

        pub fn read_u32(&mut self) -> Result<u32, MessageError> {
            let value = self.read_bytes(4)?;
            let value = u32::from_be_bytes([value[0], value[1], value[2], value[3]]);
            Ok(value)
        }

        pub fn read_u16(&mut self) -> Result<u16, MessageError> {
            let value = self.read_bytes(2)?;
            let value = u16::from_be_bytes([value[0], value[1]]);
            Ok(value)
        }

        pub fn read_bytes(&mut self, length: usize) -> Result<&'a [u8], MessageError> {
            if self.len() < length {
                return Err(MessageError::Incomplete);
            }
            let value = &self.buffer[self.cursor..self.cursor + length];
            self.cursor += length;
            Ok(value)
        }
    }

    #[derive(Debug)]
    pub struct MessageHeader {
        /// Protocol version (For future sake)
        pub version: u8,
        /// ID of the tunnel this message is from, [u32::MAX] when the
        /// tunnel is not yet initiated
        pub tunnel_id: u32,
    }

    #[derive(Debug, Error)]
    pub enum MessageError {
        #[error("unknown message type")]
        UnknownMessageType,

        #[error("message was incomplete")]
        Incomplete,
    }

    impl MessageHeader {
        pub fn read(buf: &mut MessageReader<'_>) -> Result<MessageHeader, MessageError> {
            let version = buf.read_u8()?;
            let tunnel_id = buf.read_u32()?;

            Ok(Self { version, tunnel_id })
        }

        pub fn write(&self, buf: &mut MessageWriter) {
            buf.write_u8(self.version);
            buf.write_u32(self.tunnel_id);
        }
    }

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    #[repr(u8)]
    pub enum MessageType {
        /// Client is requesting to initiate a connection
        Initiate = 0x0,

        /// Server has accepted a connection
        Initiated = 0x1,

        /// Forward a message on behalf of the player to
        /// another player
        Forward = 0x2,

        /// Message to keep the stream alive
        /// (When the connect is inactive)
        KeepAlive = 0x3,
    }

    impl TryFrom<u8> for MessageType {
        type Error = MessageError;
        fn try_from(value: u8) -> Result<Self, MessageError> {
            Ok(match value {
                0x0 => Self::Initiate,
                0x1 => Self::Initiated,
                0x2 => Self::Forward,
                0x3 => Self::KeepAlive,
                _ => return Err(MessageError::UnknownMessageType),
            })
        }
    }

    #[derive(Debug)]
    pub enum TunnelMessage {
        /// Client is requesting to initiate a connection
        Initiate {
            /// Association token to authenticate with
            association_token: String,
        },

        /// Server created and associated the tunnel
        Initiated {
            /// Unique ID for the tunnel to include in future messages
            /// to identify itself
            tunnel_id: u32,
        },

        /// Client wants to forward a message
        Forward {
            /// Local socket pool index the message was sent to.
            /// Used to map to the target within the game
            index: u8,

            /// Message contents to forward
            message: Vec<u8>,
        },

        /// Keep alive
        KeepAlive,
    }

    impl TunnelMessage {
        pub fn read(buf: &mut MessageReader<'_>) -> Result<TunnelMessage, MessageError> {
            let ty = buf.read_u8()?;
            let ty = MessageType::try_from(ty)?;

            match ty {
                MessageType::Initiate => {
                    // Get length of the association token
                    let length = buf.read_u16()? as usize;
                    let token_bytes = buf.read_bytes(length)?;
                    let token = String::from_utf8_lossy(token_bytes);
                    Ok(TunnelMessage::Initiate {
                        association_token: token.to_string(),
                    })
                }
                MessageType::Initiated => {
                    let tunnel_id = buf.read_u32()?;

                    Ok(TunnelMessage::Initiated { tunnel_id })
                }
                MessageType::Forward => {
                    let index = buf.read_u8()?;

                    // Get length of the association token
                    let length = buf.read_u16()? as usize;

                    let message = buf.read_bytes(length)?;

                    Ok(TunnelMessage::Forward {
                        index,
                        message: message.to_vec(),
                    })
                }
                MessageType::KeepAlive => Ok(TunnelMessage::KeepAlive),
            }
        }

        pub fn write(&self, buf: &mut MessageWriter) {
            match self {
                TunnelMessage::Initiate { association_token } => {
                    debug_assert!(association_token.len() < u16::MAX as usize);
                    buf.write_u8(MessageType::Initiate as u8);

                    buf.write_u16(association_token.len() as u16);
                    buf.write_bytes(association_token.as_bytes());
                }
                TunnelMessage::Initiated { tunnel_id } => {
                    buf.write_u8(MessageType::Initiated as u8);
                    buf.write_u32(*tunnel_id);
                }
                TunnelMessage::Forward { index, message } => {
                    buf.write_u8(MessageType::Forward as u8);
                    debug_assert!(message.len() < u16::MAX as usize);

                    buf.write_u8(*index);
                    buf.write_u16(message.len() as u16);
                    buf.write_bytes(message);
                }
                TunnelMessage::KeepAlive => {
                    buf.write_u8(MessageType::KeepAlive as u8);
                }
            }
        }
    }
}
