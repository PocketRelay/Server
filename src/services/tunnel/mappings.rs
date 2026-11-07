use std::{collections::HashMap, net::SocketAddr, time::Duration};

use tokio::time::Instant;

use crate::{services::sessions::AssociationId, utils::hashing::IntHashMap};

use super::{PoolId, PoolIndex, TunnelId, http_tunnel::HttpTunnelHandle};

#[derive(Clone)]
pub struct TunnelData {
    /// Association ID for the tunnel
    pub association: AssociationId,
    /// Handle to the tunnel
    pub handle: TunnelHandle,
    /// Last time a keep alive was obtained for the tunnel
    pub last_alive: Instant,
}

#[derive(Clone)]
pub enum TunnelHandle {
    /// UDP tunnels have a socket address target
    Udp(SocketAddr),

    /// HTTP tunnels have a handle to a tunnel
    Http(HttpTunnelHandle),
}

/// Mapping between tunnel ID and the inverse
#[derive(Default)]
pub struct TunnelPoolMap {
    /// Mapping associating a [TunnelId] with a [PoolIndex] within a [PoolId]
    /// that it is apart of
    tunnel_to_pool: IntHashMap<TunnelId, PoolKey>,
    /// Inverse mapping of `tunnel_to_index` for finding the handle
    /// associated to a specific pool and slot
    pool_to_tunnel: IntHashMap<PoolKey, TunnelId>,
}

impl TunnelPoolMap {
    pub fn get_by_tunnel(&self, tunnel_id: &TunnelId) -> Option<&PoolKey> {
        self.tunnel_to_pool.get(tunnel_id)
    }

    pub fn get_by_pool_key(&self, pool_key: &PoolKey) -> Option<&TunnelId> {
        self.pool_to_tunnel.get(pool_key)
    }

    pub fn insert(&mut self, tunnel_id: TunnelId, pool_key: PoolKey) {
        self.tunnel_to_pool.insert(tunnel_id, pool_key);
        self.pool_to_tunnel.insert(pool_key, tunnel_id);
    }

    pub fn remove_by_pool_key(&mut self, pool_key: &PoolKey) {
        if let Some(tunnel_id) = self.pool_to_tunnel.remove(pool_key) {
            self.tunnel_to_pool.remove(&tunnel_id);
        }
    }

    pub fn remove_by_tunnel(&mut self, tunnel_id: &TunnelId) {
        if let Some(pool_key) = self.tunnel_to_pool.remove(tunnel_id) {
            self.pool_to_tunnel.remove(&pool_key);
        }
    }
}

#[derive(Default)]
pub struct TunnelMappings {
    /// Next available tunnel ID
    next_tunnel_id: TunnelId,

    /// Mapping from [TunnelId]s to the actual [TunnelHandle] for communicating
    /// with the tunnel
    id_to_tunnel: IntHashMap<TunnelId, TunnelData>,

    /// Mapping from [AssociationId] (Client association) to [TunnelHandle]
    association_to_tunnel: HashMap<AssociationId, TunnelId>,

    /// Mapping from [AssociationId] to the game pool slot the client is in,
    /// kept even if the client doesn't have a tunnel yet so a tunnel created
    /// (or re-created) later on is still connected to the right slot
    association_to_pool: HashMap<AssociationId, PoolKey>,

    pool: TunnelPoolMap,
}

/// Represents a key that is created from a [PoolId] and [PoolIndex] combined
/// into a single value.
///
/// This allows it to be used as a key in the [IntHashMap]
#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub struct PoolKey(u64);

impl PoolKey {
    /// Creates a new pool key from its components
    pub const fn new(pool_id: PoolId, pool_index: PoolIndex) -> Self {
        Self(((pool_id as u64) << 32) | pool_index as u64)
    }

    /// Converts the key into its underlying parts
    const fn parts(&self) -> (PoolId, PoolIndex) {
        ((self.0 >> 32) as PoolId, self.0 as PoolIndex)
    }
}

impl TunnelMappings {
    /// Attempts to obtain the next available tunnel ID to allocate to
    /// a new tunnel, will return [None] if all IDs are determined to
    /// have been exhausted
    fn acquire_tunnel_id(&mut self) -> Option<TunnelId> {
        let mut addr_exhausted = 0;

        loop {
            // Acquire the tunnel ID
            let tunnel_id = self.next_tunnel_id;

            // Increase tunnel ID
            self.next_tunnel_id = self.next_tunnel_id.wrapping_add(1);

            // If the one we were issued was the last address then the next
            // address will loop around to zero
            if tunnel_id == u32::MAX {
                addr_exhausted += 1;
            }

            // Ensure the tunnel isn't already mapped
            if !self.id_to_tunnel.contains_key(&tunnel_id) {
                return Some(tunnel_id);
            }

            // If we iterated the full range of u32 twice we've definitely exhausted all possible IDs
            if addr_exhausted > 2 {
                return None;
            }
        }
    }

    pub fn tunnel_data(&self) -> impl Iterator<Item = (&TunnelId, &TunnelData)> + '_ {
        self.id_to_tunnel.iter()
    }

    /// Drops all expired/closed tunnels
    pub fn remove_dead_tunnels(&mut self, now: Instant, keep_alive_timeout: Duration) {
        // Retain all now expired tunnels
        self.id_to_tunnel.retain(|tunnel_id, data| {
            // Initial dead check based on keep alive timeout
            let mut dead = now.duration_since(data.last_alive) > keep_alive_timeout;

            // Check for closed http tunnels
            if let TunnelHandle::Http(handle) = &data.handle
                && handle.tx.is_closed()
            {
                dead = true
            }

            // Remove other tunnel mappings
            if dead {
                // Remove association mapping
                self.association_to_tunnel.remove(&data.association);
                self.pool.remove_by_tunnel(tunnel_id);
            }

            !dead
        });
    }

    /// Inserts a new tunnel into the mappings and associates it to the
    /// provided association token
