use crate::routes::auth::complete_auth;
use blaze_pk::{
    codec::{Codec, CodecResult, Reader},
    packet,
    packet::Packet,
    tag::Tag,
    types::TdfOptional,
};
use core::blaze::components::UserSessions;
use core::blaze::errors::{HandleResult, ServerError};

use crate::session::Session;
use core::blaze::codec::{NetExt, NetGroups};
use database::PlayersInterface;
use log::{debug, warn};

/// Routing function for handling packets with the `Stats` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
pub async fn route(
    session: &mut Session,
    component: UserSessions,
    packet: &Packet,
) -> HandleResult {
    match component {
        UserSessions::ResumeSession => handle_resume_session(session, packet).await,
        UserSessions::UpdateNetworkInfo => handle_update_network_info(session, packet).await,
        UserSessions::UpdateHardwareFlags => handle_update_hardware_flag(session, packet).await,
        component => {
            debug!("Got UserSessions({component:?})");
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
async fn handle_resume_session(session: &mut Session, packet: &Packet) -> HandleResult {
    let req = packet.decode::<ResumeSession>()?;
    let Some(player) = PlayersInterface::by_token(session.db(), &req.session_token).await? else {
        return session
            .response_error(packet, ServerError::InvalidSession)
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
async fn handle_update_network_info(session: &mut Session, packet: &Packet) -> HandleResult {
    let req = packet.decode::<UpdateNetworkInfo>()?;
    let groups = match req.address {
        TdfOptional::Some(_, value) => value.1,
        TdfOptional::None => {
            warn!("Client didn't provide the expected networking information");
            return session.response_empty(packet).await;
        }
    };
    session.set_network_info(groups, req.nqos).await;
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
async fn handle_update_hardware_flag(session: &mut Session, packet: &Packet) -> HandleResult {
    let req = packet.decode::<UpdateHWFlagReq>()?;
    session.set_hardware_flag(req.hardware_flag);
    session.response_empty(packet).await
}
