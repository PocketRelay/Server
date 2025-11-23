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
            let mut dead = data.last_alive.duration_since(now) > keep_alive_timeout;

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
    ///
    /// Creates and returns a tunnel ID if [None] is returned no tunnel ID could be created
    pub fn insert_tunnel(
        &mut self,
        association: AssociationId,
        tunnel: TunnelData,
    ) -> Option<TunnelId> {
        let tunnel_id = self.acquire_tunnel_id()?;

        // Insert the tunnel into the mappings
        self.id_to_tunnel.insert(tunnel_id, tunnel);
        self.association_to_tunnel.insert(association, tunnel_id);

        Some(tunnel_id)
    }

    pub fn update_tunnel_handle(&mut self, tunnel_id: TunnelId, handle: TunnelHandle) {
        if let Some(tunnel_data) = self.id_to_tunnel.get_mut(&tunnel_id) {
            tunnel_data.handle = handle;
        }
    }

    /// Updates the last-alive instant for the tunnel
    pub fn update_tunnel_last_alive(&mut self, tunnel_id: TunnelId, last_alive: Instant) {
        if let Some(tunnel_data) = self.id_to_tunnel.get_mut(&tunnel_id) {
            tunnel_data.last_alive = last_alive;
        }
    }

    pub fn get_association_tunnel(&self, association: &AssociationId) -> Option<TunnelId> {
        self.association_to_tunnel.get(association).copied()
    }

    /// Attempts to associate the tunnel from `address` to the provided
    /// `pool_id` and `pool_index` if there is a tunnel connected to
    /// `address`
    pub fn associate_pool(&mut self, tunnel_id: TunnelId, pool_id: PoolId, pool_index: PoolIndex) {
        let key = PoolKey::new(pool_id, pool_index);
        self.pool.insert(tunnel_id, key);
    }

    /// Removes the association between a [PoolKey] and a [TunnelId] if
    /// one is present
    pub fn dissociate_pool(&mut self, pool_id: PoolId, pool_index: PoolIndex) {
        self.pool
            .remove_by_pool_key(&PoolKey::new(pool_id, pool_index));
    }

    /// Get the pool key of a tunnel
    pub fn get_tunnel_pool_key(&self, tunnel_id: TunnelId) -> Option<(PoolId, PoolIndex)> {
        self.pool
            .get_by_tunnel(&tunnel_id)
            .map(|pool_key| pool_key.parts())
    }

    /// Get a tunnel using a pool key
    pub fn get_tunnel_by_pool_key(&self, pool_key: &PoolKey) -> Option<&TunnelHandle> {
        let tunnel_id = self.pool.get_by_pool_key(pool_key)?;
        self.id_to_tunnel
            .get(tunnel_id)
            .map(|tunnel| &tunnel.handle)
    }
}
