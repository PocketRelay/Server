use crate::{session::SessionData, utils::types::PlayerID};
use tdf::{TdfDeserialize, TdfSerialize, TdfTyped};

use super::{util::PING_SITE_ALIAS, NetworkAddress, QosNetworkData};

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

#[derive(TdfTyped)]
#[tdf(group)]
pub struct UserSessionExtendedData<'a> {
    session_data: &'a SessionData,
}

impl TdfSerialize for UserSessionExtendedData<'_> {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.group_body(|w| {
            w.tag_ref(b"ADDR", &self.session_data.net.addr); // Network address
            w.tag_str(b"BPS", PING_SITE_ALIAS); // Best ping site alias
            w.tag_str_empty(b"CTY"); // Country
            w.tag_var_int_list_empty(b"CVAR"); // Client data
            w.tag_map_tuples(b"DMAP", &[(0x70001, 0x409a)]); // Data map
            w.tag_owned(b"HWFG", self.session_data.net.hardware_flags) // Hardware flags
        })
    }
}
