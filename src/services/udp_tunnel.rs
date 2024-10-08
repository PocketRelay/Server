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

pub async fn start_udp_tunnel(
    tunnel_addr: SocketAddr,
    service: Arc<UdpTunnelService>,
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
pub async fn accept_messages(service: Arc<UdpTunnelService>, socket: Arc<UdpSocket>) {
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
pub async fn keep_alive(service: Arc<UdpTunnelService>, socket: Arc<UdpSocket>) {
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
            service
                .mappings
                .read()
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
                let socket = socket.clone();

                async move { socket.send_to(&buffer, addr).await }
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

/// UDP tunneling service
pub struct UdpTunnelService {
    /// Next available tunnel ID
    next_tunnel_id: AtomicU32,
    /// Tunneling mapping data
    mappings: RwLock<TunnelMappings>,
    /// Access to the session service for exchanging
    /// association tokens
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

    /// Updates the [SocketAddr] for an existing tunnel when the address has changed
    fn update_tunnel_addr(&mut self, tunnel_id: TunnelId, tunnel_addr: SocketAddr) {
        if let Some(tunnel_data) = self.id_to_tunnel.get_mut(&tunnel_id) {
            tunnel_data.addr = tunnel_addr;
        }
    }

    /// Updates the last-alive instant for the tunnel
    fn update_tunnel_last_alive(&mut self, tunnel_id: TunnelId, last_alive: Instant) {
        if let Some(tunnel_data) = self.id_to_tunnel.get_mut(&tunnel_id) {
            tunnel_data.last_alive = last_alive;
        }
    }

    /// Checks if the provided `tunnel_id` is already in use
    fn tunnel_exists(&self, tunnel_id: TunnelId) -> bool {
        self.id_to_tunnel.contains_key(&tunnel_id)
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

impl UdpTunnelService {
    pub fn new(sessions: Arc<Sessions>) -> Self {
        Self {
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

    /// Attempts to obtain the next available tunnel ID to allocate to
    /// a new tunnel, will return [None] if all IDs are determined to
    /// have been exhausted
    fn acquire_tunnel_id(&self) -> Option<TunnelId> {
        let mut addr_exhausted = 0;

        // Attempt to acquire an available tunnel ID
        // Hold read lock while searching
        let mappings = &*self.mappings.read();

        loop {
            // Acquire the tunnel ID
            let tunnel_id = self.next_tunnel_id.fetch_add(1, Ordering::AcqRel);

            // If the one we were issued was the last address then the next
            // address will loop around to zero
            if tunnel_id == u32::MAX {
                addr_exhausted += 1;
            }

            // Ensure the tunnel isn't already mapped
            if !mappings.tunnel_exists(tunnel_id) {
                return Some(tunnel_id);
            }

            // If we iterated the full range of u32 twice we've definitely exhausted all possible IDs
            if addr_exhausted > 2 {
                return None;
            }
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
            self.mappings.write().update_tunnel_addr(tunnel_id, addr);
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

                // Attempt to acquire an available tunnel ID
                let tunnel_id = match self.acquire_tunnel_id() {
                    Some(value) => value,
                    // Cannot allocate the tunnel an ID
                    None => {
                        error!("failed to allocate a tunnel ID: exhausted");
                        return;
                    }
                };

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

                _ = socket.send_to(&buffer, addr).await;
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
