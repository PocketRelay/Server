use std::env::var;
use std::ops::Deref;
use blaze_pk::{Codec, CodecError, CodecResult, encode_empty_str, encode_field, encode_zero, OpaquePacket, packet, Packets, Reader, Tag, ValueType};
use log::debug;
use crate::blaze::components::Authentication;
use crate::blaze::errors::{BlazeError, HandleResult, LoginError, LoginErrorRes};
use crate::blaze::routes::response_error;
use crate::blaze::{Session, SessionData};
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

    if player.session_token.ne(&token) {
        return Err(login_error(packet, LoginError::InvalidSession));
    }

    debug!("Silent authentication success");
    debug!("ID = {}", &player.id);
    debug!("Username = {}", &player.display_name);
    debug!("Email = {}", &player.email);

    complete_auth(session, player, true).await?;
    Ok(())
}

/// Completes the authentication process for the provided session using the provided Player
/// Model as the authenticated player.
async fn complete_auth(session: &Session, player: PlayerModel, silent: bool) -> HandleResult {
    let player = session.set_player(Some(player)).await
        .ok_or(BlazeError::MissingPlayer)?;
    let session_token = session.session_token().await?;
    let session_data = session.data.read().await;
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

/// Complex authentication result structure is manually encoded because it
/// has complex nesting and output can vary based on inputs provided
pub struct AuthRes<'a, 'b> {
    session_data: &'a SessionData,
    player: &'b PlayerModel,
    session_token: String,
    silent: bool,
}

impl Codec for AuthRes {
    fn encode(&self, output: &mut Vec<u8>) {
        let silent = self.silent;
        if silent {
            encode_zero!(output, AGUP);
        }

        encode_empty_str!(output, LDHT);
        encode_zero!(output, NTOS);
        encode_field!(output, PCTK, &self.session_token, String);

        #[inline]
        fn encode_persona(player: &PlayerModel, output: &mut Vec<u8>) {
            encode_field!(output, DSNM, &player.display_name, String);
            encode_zero!(output, LAST);
            encode_field!(output, PID, &player.id, u32);
            encode_zero!(output, STAS);
            encode_zero!(output, XREF);
            encode_zero!(output, XTYP);
            output.push(0);
        }

        if silent {
            encode_empty_str!(output, PRIV);
            Tag::encode_from("SESS", &ValueType::Group, output);
            encode_field!(output, BUID, &self.player.id, u32);
            encode_zero!(output, FRST);
            encode_field!(output, KEY, &self.session_token, String);
            encode_zero!(output, LLOG);
            encode_field!(output, MAIL, &self.player.email, String);
            Tag::encode_from("PDTL", &ValueType::Group, output);
            encode_persona(&self.player, output);
            encode_field!(output, UID, &self.player.id, u32);
            output.push(0);
        } else {
            Tag::encode_from("PLST", &ValueType::List, output);
            ValueType::Group.encode(output);
            output.push(1);
            encode_persona(&self.player, output);

            encode_empty_str!(output, PRIV);
            encode_field!(output, SKEY, &self.session_token, String);
        }
        encode_zero!(output, SPAM);
        encode_empty_str!(output, THST);
        encode_empty_str!(output, TSUI);
        encode_empty_str!(output, TURI);
        if !silent {
            encode_field!(output, UID, &self.player.id, u32);
        }
    }

    fn decode(_: &mut Reader) -> CodecResult<Self> {
        Err(CodecError::InvalidAction("Not allowed to decode AuthRes"))
    }
}