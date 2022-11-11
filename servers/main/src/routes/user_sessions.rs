use std::net::{IpAddr, SocketAddr};

use crate::routes::auth::complete_auth;
use blaze_pk::{packet, Codec, CodecResult, OpaquePacket, Reader, Tag, TdfOptional};
use core::blaze::components::UserSessions;
use core::blaze::errors::{HandleResult, ServerError};
use core::blaze::session::SessionArc;
use core::blaze::shared::{NetAddress, NetExt, NetGroups};
use database::PlayersInterface;
use log::{debug, warn};
use utils::ip::public_address;

/// Routing function for handling packets with the `Stats` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
pub async fn route(
    session: &SessionArc,
    component: UserSessions,
    packet: &OpaquePacket,
) -> HandleResult {
    match component {
        UserSessions::ResumeSession => handle_resume_session(session, packet).await,
        UserSessions::UpdateNetworkInfo => handle_update_network_info(session, packet).await,
        UserSessions::UpdateHardwareFlags => handle_update_hardware_flag(session, packet).await,
        component => {
            debug!("Got UserSessions({component:?})");
            packet.debug_decode()?;
            session.response_empty(packet).await
        }
    }
}

packet! {
    struct ResumeSession {
        SKEY session_token: String
    }
}

/// Handles resuming a session with the provides session token
///
/// # Structure
/// *To be recorded*
async fn handle_resume_session(session: &SessionArc, packet: &OpaquePacket) -> HandleResult {
    let req = packet.contents::<ResumeSession>()?;
    let Some(player) = PlayersInterface::by_token(session.db(), &req.session_token).await? else {
        return session
            .response_error_empty(packet, ServerError::InvalidSession)
            .await;
    };
    complete_auth(session, packet, player, true).await
}

#[derive(Debug)]
struct UpdateNetworkInfo {
    address: TdfOptional<NetGroups>,
    nqos: NetExt,
}

impl Codec for UpdateNetworkInfo {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let address = Tag::expect(reader, "ADDR")?;
        let nqos = Tag::expect(reader, "NQOS")?;

        Ok(Self { address, nqos })
    }
}

/// Handles updating the stored networking information for the current session
/// this is required for clients to be able to connect to each-other
///
/// # Structure
/// ```
/// packet(Components.USER_SESSIONS, Commands.UPDATE_NETWORK_INFO, 0x0, 0x8) {
///   optional("ADDR",
///   0x2,
///     group("VALU") {
///       +group("EXIP") {
///         number("IP", 0x0)
///         number("PORT", 0x0)
///       }
///       +group("INIP") {
///         number("IP", 0x0)
///         number("PORT", 0x0)
///       }
///     }
///   )
///   map("NLMP", mapOf(
///     "ea-sjc" to 0x9c,
///     "rs-iad" to 0xfff0fff,
///     "rs-lhr" to 0xfff0fff,
///   ))
///   +group("NQOS") {
///     number("DBPS", 0x0)
///     number("NATT", 0x4)
///     number("UBPS", 0x0)
///   }
/// }
/// ```
async fn handle_update_network_info(session: &SessionArc, packet: &OpaquePacket) -> HandleResult {
    let req = packet.contents::<UpdateNetworkInfo>()?;
    let groups = match req.address {
        TdfOptional::Some(_, value) => value.1,
        TdfOptional::None => {
            warn!("Client didn't provide the expected networking information");
            return session.response_empty(packet).await;
        }
    };

    {
        let session_data = &mut *session.data.write().await;
        let mut net = &mut session_data.net;
        net.is_unset = false;
        net.ext = req.nqos;
        net.groups = groups;
        update_missing_external(session, &mut net.groups).await;
        debug!("Updating networking:\n{:#?}", net)
    }

    session.update_client().await;
    debug!("Done update networking");
    session.response_empty(packet).await
}

pub async fn update_missing_external(session: &SessionArc, groups: &mut NetGroups) {
    let external = &mut groups.external;
    if external.0.is_invalid() || external.1 == 0 {
        // Match port with internal address
        external.1 = groups.internal.1;
        external.0 = get_address_from(&session.addr).await;
    }
}

pub async fn get_address_from(value: &SocketAddr) -> NetAddress {
    let ip = value.ip();
    if let IpAddr::V4(value) = ip {
        // Value is local or private
        if value.is_loopback() || value.is_private() {
            if let Some(public_addr) = public_address().await {
                return NetAddress::from_ipv4(&public_addr);
            }
        }
        let value = format!("{}", value);
        NetAddress::from_ipv4(&value)
    } else {
        // Don't know how to handle IPv6 addresses
        return NetAddress(0);
    }
}

packet! {
    struct UpdateHWFlagReq {
        HWFG hardware_flag: u16,
    }
}

/// Handles updating the stored hardware flag with the client provided hardware flag
///
/// # Structure
/// ```
/// packet(Components.USER_SESSIONS, Commands.UPDATE_HARDWARE_FLAGS, 0x0, 0x16) {
///   number("HWFG", 0x0)
/// }
/// ```
async fn handle_update_hardware_flag(session: &SessionArc, packet: &OpaquePacket) -> HandleResult {
    let req = packet.contents::<UpdateHWFlagReq>()?;
    {
        let session_data = &mut *session.data.write().await;
        session_data.hardware_flag = req.hardware_flag;
    }
    session.update_client().await;
    debug!("Done updating hardware flag");
    session.response_empty(packet).await
}
