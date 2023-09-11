use crate::utils::{
    models::{NetworkAddress, QosNetworkData},
    types::PlayerID,
};
use tdf::TdfDeserialize;

#[derive(Debug, Clone)]
#[repr(u16)]
#[allow(unused)]
pub enum UserSessionsError {
    UserNotFound = 0xb,
}

/// Structure for a request to resume a session using a session token
#[derive(TdfDeserialize)]
pub struct ResumeSessionRequest {
    /// The session token to use
    #[tdf(tag = "SKEY")]
    pub session_token: String,
}

/// Structure for a request to update the network info of the
/// current session
#[derive(TdfDeserialize)]
pub struct UpdateNetworkRequest {
    /// The client address net groups
    #[tdf(tag = "ADDR")]
    pub address: NetworkAddress,
    /// The client Quality of Service data
    #[tdf(tag = "NQOS")]
    pub qos: QosNetworkData,
}

/// Structure for request to update the hardware flags of the
/// current session
#[derive(TdfDeserialize)]
pub struct HardwareFlagRequest {
    /// The hardware flag value
    #[tdf(tag = "HWFG")]
    pub hardware_flag: u16,
}

#[derive(TdfDeserialize)]
pub struct LookupRequest {
    #[tdf(tag = "ID")]
    pub player_id: PlayerID,
}
