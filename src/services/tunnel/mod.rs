use std::sync::Arc;

use http_tunnel::HttpTunnelService;
use udp_tunnel::UdpTunnelService;

use crate::utils::types::GameID;

use super::sessions::{AssociationId, Sessions};

pub mod http_tunnel;
pub mod udp_tunnel;

/// ID for a tunnel
pub type TunnelId = u32;
/// Index into a pool of tunnels
pub type PoolIndex = u8;
/// ID of a pool
pub type PoolId = GameID;

pub struct TunnelService {
    http: HttpTunnelService,
    udp: UdpTunnelService,
}

impl TunnelService {
    pub fn new(sessions: Arc<Sessions>) -> Self {
        let http = HttpTunnelService::default();
        let udp = UdpTunnelService::new(sessions);

        Self { http, udp }
    }

    pub fn associate_pool(
        &self,
        association: AssociationId,
        pool_id: PoolId,
        pool_index: PoolIndex,
    ) {
        self.http.associate_pool(association, pool_id, pool_index);
        self.udp.associate_pool(association, pool_id, pool_index);
    }

    pub fn dissociate_pool(&self, pool_id: PoolId, pool_index: PoolIndex) {
        self.http.dissociate_pool(pool_id, pool_index);
        self.udp.dissociate_pool(pool_id, pool_index);
    }

    pub fn associate_tunnel_http(&self, association: AssociationId, tunnel_id: TunnelId) {
        self.http.associate_tunnel(association, tunnel_id);
    }

    pub fn dissociate_tunnel_http(&self, tunnel_id: TunnelId) {
        self.http.dissociate_tunnel(tunnel_id);
    }
}
