use std::collections::HashMap;

use tokio::time::Instant;

use crate::{services::sessions::AssociationId, utils::hashing::IntHashMap};

use super::{PoolId, PoolIndex, TunnelId};

#[derive(Clone)]
pub struct TunnelData<Handle> {
    /// Association ID for the tunnel
    pub association: AssociationId,
    /// Handle to the tunnel
    pub handle: Handle,
    /// Last time a keep alive was obtained for the tunnel
    pub last_alive: Instant,
}

pub struct TunnelMappings<Handle> {
    /// Next available tunnel ID
    next_tunnel_id: TunnelId,

    /// Mapping from [TunnelId]s to the actual [TunnelHandle] for communicating
    /// with the tunnel
    id_to_tunnel: IntHashMap<TunnelId, TunnelData<Handle>>,

    /// Mapping from [AssociationId] (Client association) to [TunnelHandle]
    association_to_tunnel: HashMap<AssociationId, TunnelId>,

    /// Mapping associating a [TunnelId] with a [PoolIndex] within a [PoolId]
    /// that it is apart of
    tunnel_to_index: IntHashMap<TunnelId, PoolKey>,
    /// Inverse mapping of `tunnel_to_index` for finding the handle
    /// associated to a specific pool and slot
    index_to_tunnel: IntHashMap<PoolKey, TunnelId>,
}

impl<Handle> Default for TunnelMappings<Handle> {
    fn default() -> Self {
        Self {
            next_tunnel_id: Default::default(),
            id_to_tunnel: Default::default(),
            association_to_tunnel: Default::default(),
            tunnel_to_index: Default::default(),
            index_to_tunnel: Default::default(),
        }
    }
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

impl<Handle: Clone> TunnelMappings<Handle> {
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

    pub fn tunnel_data(&self) -> Vec<(TunnelId, TunnelData<Handle>)> {
        self.id_to_tunnel
            .iter()
            .map(|(tunnel_id, value)| (*tunnel_id, value.clone()))
            .collect()
    }

    /// Inserts a new tunnel into the mappings and associates it to the
    /// provided association token
    ///
    /// Creates and returns a tunnel ID if [None] is returned no tunnel ID could be created
    pub fn insert_tunnel(
        &mut self,
        association: AssociationId,
        tunnel: TunnelData<Handle>,
    ) -> Option<TunnelId> {
        let tunnel_id = self.acquire_tunnel_id()?;

        // Insert the tunnel into the mappings
        self.id_to_tunnel.insert(tunnel_id, tunnel);
        self.association_to_tunnel.insert(association, tunnel_id);

        Some(tunnel_id)
    }

    pub fn update_tunnel_handle(&mut self, tunnel_id: TunnelId, handle: Handle) {
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

    /// Attempts to associate the tunnel from `address` to the provided
    /// `pool_id` and `pool_index` if there is a tunnel connected to
    /// `address`
    pub fn associate_pool(
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
    pub fn get_tunnel_route(
        &self,
        tunnel_id: TunnelId,
        pool_index: PoolIndex,
    ) -> Option<(Handle, PoolIndex)> {
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
    pub fn dissociate_tunnel(&mut self, tunnel_id: TunnelId) {
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
    pub fn dissociate_pool(&mut self, pool_id: PoolId, pool_index: PoolIndex) {
        if let Some(tunnel_id) = self
            .index_to_tunnel
            .remove(&PoolKey::new(pool_id, pool_index))
        {
            self.tunnel_to_index.remove(&tunnel_id);
        }
    }
}
