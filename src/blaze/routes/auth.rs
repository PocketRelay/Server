use blaze_pk::{OpaquePacket, packet};
use log::{debug, info};
use crate::blaze::components::Authentication;
use crate::blaze::errors::{LoginError, LoginErrorRes};
use crate::blaze::routes::{HandleResult, response_error};
use crate::blaze::Session;
use crate::database::interface::players::find_by_id;

/// Routing function for handling packets with the `Authentication` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
pub async fn route(session: &Session, component: Authentication, packet: &OpaquePacket) -> HandleResult {
    match component {
        Authentication::SilentLogin => handle_silent_login(session, packet).await,
        component => {
            println!("Got {component:?}");
            packet.debug_decode()?;
            Ok(None)
        }
    }
}


packet! {
    struct SilentLoginReq {
        AUTH token: String,
        PID id: u32,
    }
}

async fn handle_silent_login(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let silent_login = packet.contents::<SilentLoginReq>()?;
    let id = silent_login.id;
    let token = silent_login.token;

    debug!("Attempted silent authentication: {id} ({token})");

    let player = match find_by_id(session.db(), id).await? {
        Some(player) => player,
        None => return response_error(packet, LoginError::InvalidAccount, LoginErrorRes::default())
    };

    let is_eq = match &player.session_token {
        Some(session_token) => session_token.eq(&token),
        None => false
    };

    if !is_eq {
        return response_error(packet, LoginError::InvalidSession, LoginErrorRes::default());
    }

    Ok(None)
}