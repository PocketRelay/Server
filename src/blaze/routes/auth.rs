use std::ops::Deref;
use blaze_pk::{OpaquePacket, packet, Packets};
use log::debug;
use crate::blaze::components::Authentication;
use crate::blaze::errors::{BlazeError, HandleResult, LoginError, LoginErrorRes};
use crate::blaze::Session;
use crate::blaze::shared::AuthRes;
use crate::database::entities::PlayerModel;
use crate::database::interface::players;

/// Routing function for handling packets with the `Authentication` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
pub async fn route(session: &Session, component: Authentication, packet: &OpaquePacket) -> HandleResult {
    match component {
        Authentication::SilentLogin => handle_silent_login(session, packet).await,
        Authentication::Logout => handle_logout(session, packet).await,
        component => {
            println!("Got {component:?}");
            packet.debug_decode()?;
            Ok(())
        }
    }
}


packet! {
    struct SilentLoginReq {
        AUTH token: String,
        PID id: u32,
    }
}

/// Creates a new blaze error response from the provided login error
fn login_error(packet: &OpaquePacket, error: LoginError) -> BlazeError {
    BlazeError::Response(Packets::error(packet, error, &LoginErrorRes::default()))
}

/// Handles silent authentication from a client (Token based authentication) If the token provided
/// by the client is correct the session is updated accordingly to match the player
/// # Structure
/// ```
/// packet(Components.AUTHENTICATION, Commands.SILENT_LOGIN, 0x0, 0x6) {
///   text("AUTH", "128 CHAR TOKEN OMITTED")
///   number("PID", 0x1)
///   number("TYPE", 0x2)
/// }
/// ```
///
async fn handle_silent_login(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let silent_login = packet.contents::<SilentLoginReq>()?;
    let id = silent_login.id;
    let token = silent_login.token;

    debug!("Attempted silent authentication: {id} ({token})");

    let player = players::find_by_id(session.db(), id)
        .await?
        .ok_or_else(|| login_error(packet, LoginError::InvalidSession))?;

    if player.session_token.ne(&Some(token)) {
        return Err(login_error(packet, LoginError::InvalidSession));
    }

    debug!("Silent authentication success");
    debug!("ID = {}", &player.id);
    debug!("Username = {}", &player.display_name);
    debug!("Email = {}", &player.email);

    complete_auth(session, packet, player, true).await?;
    Ok(())
}

/// Completes the authentication process for the provided session using the provided Player
/// Model as the authenticated player.
async fn complete_auth(session: &Session, packet: &OpaquePacket, player: PlayerModel, silent: bool) -> HandleResult {
    session.set_player(Some(player)).await;
    let session_token = session.session_token().await?;
    let session_data = session.data.read().await;
    let player = session_data.expect_player()?;
    let response = AuthRes {
        session_data: session_data.deref(),
        session_token,
        player,
        silent,
    };

    session.response(packet, &response).await?;
    if silent {
        session.update_for(session).await?;
    }
    Ok(())
}

/// Handles logging out by the client this removes any current player data from the
/// session and updating anything that depends on the session having a player.
///
/// # Structure
/// ```
/// packet(Components.AUTHENTICATION, Commands.LOGOUT, 0x0, 0x7) {}
/// ```
async fn handle_logout(session: &Session, packet: &OpaquePacket) -> HandleResult {
    debug!("Logging out for session:");
    debug!("ID = {}", &session.id);
    session.set_player(None).await;
    session.response_empty(packet).await
}

