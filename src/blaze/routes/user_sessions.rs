use std::ops::DerefMut;
use blaze_pk::{OpaquePacket, packet, TdfMap, TdfOptional};
use log::{debug, warn};
use crate::blaze::components::UserSessions;
use crate::blaze::errors::{HandleResult, LoginError};
use crate::blaze::routes::auth::{complete_auth, login_error};
use crate::blaze::routes::util::QOSS_KEY;
use crate::blaze::Session;
use crate::blaze::shared::{NetExt, NetGroups};
use crate::database::interface::players::find_by_session;

/// Routing function for handling packets with the `Stats` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
pub async fn route(session: &Session, component: UserSessions, packet: &OpaquePacket) -> HandleResult {
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
async fn handle_resume_session(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let req = packet.contents::<ResumeSession>()?;
    let player = find_by_session(session.db(), &req.session_token)
        .await?
        .ok_or_else(|| login_error(packet, LoginError::InvalidSession))?;
    complete_auth(session, packet, player, true).await
}

packet! {
    struct UpdateNetworkInfo {
        ADDR address: TdfOptional<NetGroups>,
        NLMP nlmp: TdfMap<String, u32>,
        NQOS nqos: NetExt
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
///         number("IP", 0xc0a8014e)
///         number("PORT", 0xe4b)
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
async fn handle_update_network_info(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let mut req = packet.contents::<UpdateNetworkInfo>()?;
    let groups = match req.address {
        TdfOptional::Some(_, value) => value.1,
        TdfOptional::None => {
            warn!("Client didn't provide the expected networking information");
            return session.response_empty(packet).await
        }
    };

    let pslm = req.nlmp.take(QOSS_KEY)
        .unwrap_or(0xfff0fff);

    let mut session_data = session.data.write().await;
    let session_data = session_data.deref_mut();
    session_data.pslm = pslm;

    let mut net = &mut session_data.net;
    net.is_unset = false;
    net.ext = req.nqos;
    net.groups = groups;

    session.response_empty(packet).await
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
async fn handle_update_hardware_flag(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let req = packet.contents::<UpdateHWFlagReq>()?;

    let mut session_data = session.data.write().await;
    (*session_data).hardware_flag = req.hardware_flag;

    session.response_empty(packet).await
}