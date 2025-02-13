use std::sync::Arc;

use http_tunnel::HttpTunnelService;
use udp_tunnel::UdpTunnelService;

use crate::utils::types::GameID;

use super::sessions::{AssociationId, Sessions};

pub mod http_tunnel;
pub mod mappings;
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
        self.http
            .mappings
            .write()
            .associate_pool(association, pool_id, pool_index);
        self.udp
            .mappings
            .write()
            .associate_pool(association, pool_id, pool_index);
    }

    pub fn dissociate_pool(&self, pool_id: PoolId, pool_index: PoolIndex) {
        self.http
            .mappings
            .write()
            .dissociate_pool(pool_id, pool_index);
        self.udp
            .mappings
            .write()
            .dissociate_pool(pool_id, pool_index);
    }

    pub fn dissociate_tunnel_http(&self, tunnel_id: TunnelId) {
        self.http.mappings.write().dissociate_tunnel(tunnel_id);
    }
}
