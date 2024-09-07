use super::sessions::{AssociationId, Sessions};
use crate::utils::{hashing::IntHashMap, types::GameID};
use log::{debug, error};
use parking_lot::RwLock;
use pocket_relay_udp_tunnel::{deserialize_message, serialize_message, TunnelMessage};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    net::UdpSocket,
    task::JoinSet,
    time::{interval_at, Instant, MissedTickBehavior},
};

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
        // Receive the message bytes
        let (size, addr) = match service.socket.recv_from(&mut buffer).await {
            Ok(value) => value,
            Err(err) => {
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

        // Handle the message through a background task
        let service = service.clone();

        // Handle the message in its own task
        tokio::spawn(async move {
            service
                .handle_message(tunnel_id, packet.message, addr)
                .await;
        });
    }
}

/// Delay between each keep-alive packet
const KEEP_ALIVE_DELAY: Duration = Duration::from_secs(5);

/// When this duration elapses between keep-alive checks for a connection
/// the connection is considered to be dead
const KEEP_ALIVE_TIMEOUT: Duration = Duration::from_secs(60);

/// Background task that sends out keep alive messages to all the sockets connected
/// to the tunnel system. Removes inactive and dead connections
pub async fn keep_alive(service: Arc<TunnelService>) {
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
        let tunnels: Vec<(TunnelId, SocketAddr, Instant)> = {
            let mappings = &*service.mappings.read();

            mappings
                .id_to_tunnel
                .iter()
                .map(|(tunnel_id, value)| (*tunnel_id, value.addr, value.last_alive))
                .collect()
        };

        // Don't need to tick if theres no tunnels available
        if tunnels.is_empty() {
            continue;
        }

        let mut expired_tunnels: Vec<TunnelId> = Vec::new();

        // Send out keep-alive messages for any tunnels that aren't expired
        for (tunnel_id, addr, last_alive) in tunnels {
            let last_alive = last_alive.duration_since(now);
            if last_alive > KEEP_ALIVE_TIMEOUT {
                expired_tunnels.push(tunnel_id);
                continue;
            }

            let buffer = serialize_message(tunnel_id, &TunnelMessage::KeepAlive);

            // Spawn the task to send the keep-alive message
            send_task_set.spawn({
                let service = service.clone();

                async move { service.socket.send_to(&buffer, addr).await }
            });
        }

        // Join all keep alive tasks
        while send_task_set.join_next().await.is_some() {}

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
                let tunnel_id = self.next_tunnel_id.fetch_add(1, Ordering::AcqRel);

                self.mappings
                    .write()
                    .associate_tunnel(association, tunnel_id);

                // Store the tunnel mapping
                self.mappings.write().insert_tunnel(
                    tunnel_id,
                    TunnelData {
                        addr,
                        association,
                        last_alive: Instant::now(),
                    },
                );

                let buffer = serialize_message(tunnel_id, &TunnelMessage::Initiated { tunnel_id });

                self.socket.send_to(&buffer, addr).await.unwrap();
            }
            TunnelMessage::Initiated { .. } => {
                // Server shouldn't be receiving this message... ignore it
            }
            TunnelMessage::Forward { index, message } => {
                // Get the path through the tunnel
                let (target_addr, index) = match self.get_tunnel_route(tunnel_id, index) {
                    Some(value) => value,
                    // Don't have a tunnel to send the message through
                    None => {
                        return;
                    }
                };

                let buffer =
                    serialize_message(tunnel_id, &TunnelMessage::Forward { index, message });

                self.socket.send_to(&buffer, target_addr).await.unwrap();
            }
            TunnelMessage::KeepAlive => {
                // Ack keep alive
            }
        }
    }
}
