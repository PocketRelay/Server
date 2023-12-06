use std::sync::Arc;

use crate::{
    database::entities::Player,
    session::NetData,
    utils::{components::game_manager::GAME_TYPE, types::PlayerID},
};
use bitflags::bitflags;
use serde::Serialize;
use tdf::{ObjectId, TdfDeserialize, TdfMap, TdfSerialize, TdfTyped};

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
    /// Latency to the different ping sites
    #[tdf(tag = "NLMP")]
    pub ping_site_latency: TdfMap<String, u32>,
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
pub struct UserSessionExtendedDataUpdate {
    #[tdf(tag = "DATA")]
    pub data: UserSessionExtendedData,
    #[tdf(tag = "USID")]
    pub user_id: PlayerID,
}

#[derive(TdfTyped)]
#[tdf(group)]
pub struct UserSessionExtendedData {
    /// Networking data for the session
    pub net: Arc<NetData>,
    /// ID of the game the player is in (if present)
    pub game: Option<u32>,
}

impl TdfSerialize for UserSessionExtendedData {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        const DMAP_LEADERBOARD_N7_RATING: u32 = 0x70001;
        // TODO: Maybe actually load this value
        const DMAP_LEADERBOARD_N7_RATING_VALUE: u32 = 100;

        w.group_body(|w| {
            // Network address
            w.tag_ref(b"ADDR", &self.net.addr);
            // Best ping site alias
            w.tag_str(b"BPS", PING_SITE_ALIAS);
            // Country
            w.tag_str_empty(b"CTY");
            // Client data
            w.tag_var_int_list_empty(b"CVAR");
            // Data map (Custom player data integer keyed)
            w.tag_map_tuples(
                b"DMAP",
                &[
                    // The players n7 rating
                    (DMAP_LEADERBOARD_N7_RATING, DMAP_LEADERBOARD_N7_RATING_VALUE),
                ],
            );
            // Hardware flags
            w.tag_owned(b"HWFG", self.net.hardware_flags.bits());
            // Ping server latency list
            w.tag_list_slice(b"PSLM", &self.net.ping_site_latency);
            // Quality of service data
            w.tag_ref(b"QDAT", &self.net.qos);
            // User info attributes
            w.tag_owned(b"UATT", 0u8);

            if let Some(game) = self.game {
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

impl<'a> UserIdentification<'a> {
    pub fn from_player(player: &'a Player) -> Self {
        Self {
            id: player.id,
            name: &player.display_name,
        }
    }
}

impl TdfSerialize for UserIdentification<'_> {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.group_body(|w| {
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
        });
    }
}

#[derive(TdfSerialize)]
pub struct NotifyUserAdded<'a> {
    /// The user session data
    #[tdf(tag = "DATA")]
    pub session_data: UserSessionExtendedData,
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
    pub player: Arc<Player>,
    pub extended_data: UserSessionExtendedData,
}

impl TdfSerialize for LookupResponse {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        // The user session extended data
        w.tag_ref(b"EDAT", &self.extended_data);
        w.tag_owned(b"FLGS", UserDataFlags::ONLINE.bits());

        // The lookup user identification
        w.tag_alt(b"USER", UserIdentification::from_player(&self.player));
    }
}
