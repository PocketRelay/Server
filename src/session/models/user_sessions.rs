use crate::{
    session::SessionData,
    utils::{components::game_manager::GAME_TYPE, types::PlayerID},
};
use bitflags::bitflags;
use serde::Serialize;
use tdf::{ObjectId, TdfDeserialize, TdfSerialize, TdfTyped};

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

/// Request to update the stored networking information for a session
#[derive(TdfDeserialize)]
pub struct UpdateNetworkRequest {
    /// The client address net groups
    #[tdf(tag = "ADDR")]
    pub address: NetworkAddress,
    /// The client Quality of Service data
    #[tdf(tag = "NQOS")]
    pub qos: QosNetworkData,
}

/// Request to update the stored hardware flags for a session
#[derive(TdfDeserialize)]
pub struct UpdateHardwareFlagsRequest {
    /// The hardware flag value
    #[tdf(tag = "HWFG", into = u8)]
    pub hardware_flags: HardwareFlags,
}

bitflags! {
    #[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
    pub struct HardwareFlags: u8 {
        const NONE = 0;
        const VOIP_HEADSET_STATUS = 1;
    }
}

impl From<HardwareFlags> for u8 {
    #[inline]
    fn from(value: HardwareFlags) -> Self {
        value.bits()
    }
}

impl From<u8> for HardwareFlags {
    #[inline]
    fn from(value: u8) -> Self {
        HardwareFlags::from_bits_retain(value)
    }
}

#[derive(TdfSerialize)]
pub struct UserSessionExtendedDataUpdate<'a> {
    #[tdf(tag = "DATA")]
    pub data: UserSessionExtendedData<'a>,
    #[tdf(tag = "USID")]
    pub user_id: PlayerID,
}

#[derive(TdfTyped)]
#[tdf(group)]
pub struct UserSessionExtendedData<'a> {
    pub session_data: &'a SessionData,
}

impl TdfSerialize for UserSessionExtendedData<'_> {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.group_body(|w| {
            // Network address
            w.tag_ref(b"ADDR", &self.session_data.net.addr);
            // Best ping site alias
            w.tag_str(b"BPS", PING_SITE_ALIAS);
            // Country
            w.tag_str_empty(b"CTY");
            // Client data
            w.tag_var_int_list_empty(b"CVAR");
            // Data map
            w.tag_map_tuples(b"DMAP", &[(0x70001, 0x409a)]);
            // Hardware flags
            w.tag_owned(b"HWFG", self.session_data.net.hardware_flags.bits());
            // Ping server latency list
            w.tag_list_slice(b"PSLM", &[0xfff0fff]);
            // Quality of service data
            w.tag_ref(b"QDAT", &self.session_data.net.qos);
            // User info attributes
            w.tag_owned(b"UATT", 0u8);

            if let Some(game) = self.session_data.game {
                // Blaze object ID list
                w.tag_list_slice(b"ULST", &[ObjectId::new(GAME_TYPE, game as u64)]);
            }
        });
    }
}

#[derive(TdfTyped)]
#[tdf(group)]
pub struct UserIdentification<'a> {
    pub id: PlayerID,
    pub name: &'a str,
}

impl TdfSerialize for UserIdentification<'_> {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        // Account ID
        w.tag_owned(b"AID", self.id);
        // Account locale
        w.tag_owned(b"ALOC", 0x64654445u32);
        // External blob
        w.tag_blob_empty(b"EXBB");
        // External ID
        w.tag_zero(b"EXID");
        // Blaze ID
        w.tag_owned(b"ID", self.id);
        // Account name
        w.tag_str(b"NAME", self.name);
    }
}

#[derive(TdfSerialize)]
pub struct NotifyUserAdded<'a> {
    /// The user session data
    #[tdf(tag = "DATA")]
    pub session_data: UserSessionExtendedData<'a>,
    /// The added user identification
    #[tdf(tag = "USER")]
    pub user: UserIdentification<'a>,
}

#[derive(TdfSerialize)]
pub struct NotifyUserRemoved {
    /// The ID of the removed user
    #[tdf(tag = "BUID")]
    pub player_id: PlayerID,
}

#[derive(TdfSerialize)]
pub struct NotifyUserUpdated {
    #[tdf(tag = "FLGS", into = u8)]
    pub flags: UserDataFlags,
    /// The ID of the updated user
    #[tdf(tag = "ID")]
    pub player_id: PlayerID,
}

bitflags! {
    #[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
    pub struct UserDataFlags: u8 {
        const NONE = 0;
        const SUBSCRIBED = 1;
        const ONLINE = 2;
    }
}

impl From<UserDataFlags> for u8 {
    fn from(value: UserDataFlags) -> Self {
        value.bits()
    }
}

impl From<u8> for UserDataFlags {
    fn from(value: u8) -> Self {
        UserDataFlags::from_bits_retain(value)
    }
}

/// Request to lookup the session details of a user, see [UserIdentification]
/// for the full structure that this uses
#[derive(TdfDeserialize)]
pub struct LookupRequest {
    #[tdf(tag = "ID")]
    pub player_id: PlayerID,
}

/// User lookup response
pub struct LookupResponse {
    pub session_data: SessionData,
}

impl TdfSerialize for LookupResponse {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        // The user session extended data
        w.tag_alt(
            b"EDAT",
            UserSessionExtendedData {
                session_data: &self.session_data,
            },
        );
        w.tag_owned(b"FLGS", UserDataFlags::ONLINE.bits());

        let player = match &self.session_data.player {
            Some(value) => value,
            None => return,
        };

        // The lookup user identification
        w.tag_alt(
            b"USER",
            UserIdentification {
                id: player.id,
                name: &player.display_name,
            },
        );
    }
}
