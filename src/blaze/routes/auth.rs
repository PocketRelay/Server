use std::ops::Deref;
use blaze_pk::{Codec, CodecError, CodecResult, encode_empty_str, encode_field, encode_zero, OpaquePacket, packet, Reader, Tag, ValueType};
use log::debug;
use crate::blaze::components::Authentication;
use crate::blaze::errors::{HandleResult, LoginError, LoginErrorRes};
use crate::blaze::routes::response_error;
use crate::blaze::{Session, SessionData};
use crate::database::entities::PlayerModel;
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

async fn handle_silent_login(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let silent_login = packet.contents::<SilentLoginReq>()?;
    let id = silent_login.id;
    let token = silent_login.token;

    debug!("Attempted silent authentication: {id} ({token})");

    let player = match find_by_id(session.db(), id).await? {
        Some(player) => player,
        None => return session.response_error(packet, LoginError::InvalidAccount, &LoginErrorRes::default())
    };

    let is_eq = match &player.session_token {
        Some(session_token) => session_token.eq(&token),
        None => false
    };

    if !is_eq {
        return session.response_error(packet, LoginError::InvalidSession, &LoginErrorRes::default());
    }

    let player = {
        let mut session_data = session.data.write().await;
        session_data.player.insert(player)
    };

    let session_token = session.session_token().await?;
    let session_data = session.data.read().await;

    let response = AuthRes {
        session,
        session_data: session_data.deref(),
        session_token,
        player,
        silent: true
    };

    session.response(packet, &response).await?;
    session.update_for(session).await?;

    Ok(())
}

pub struct AuthRes<'a, 'b> {
    session: &'a Session,
    session_data: &'a SessionData,
    player: &'b PlayerModel,
    session_token: String,
    silent: bool
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