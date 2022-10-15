use std::sync::Arc;
use blaze_pk::{CodecError, OpaquePacket, packet};
use derive_more::From;
use crate::{AppContext, Authentication, Components};
use crate::blaze::Session;
use super::routes;

#[derive(Debug, From)]
pub enum HandleError {
    CodecError(CodecError)
}

type HandleResult = Result<(), HandleError>;

pub async fn route(context: Arc<Session>, component: Components, packet: OpaquePacket) -> HandleResult {
    packet.debug_decode()?;
    match component {
        Components::Authentication(value) => match value {
            Authentication::Login => { },
            Authentication::SilentLogin => {}
            Authentication::ListDeviceAccounts => {}
            Authentication::XboxCreateAccount => {}
            Authentication::OriginLogin => {}
            Authentication::XboxAssociateAccount => {}
            Authentication::XboxLogin => {}
            Authentication::PS3CreateAccount => {}
            Authentication::PS3AssociateAccount => {}
            Authentication::PS3Login => {}
            Authentication::ValidateSessionKey => {}
            Authentication::CreateWalUserSession => {}
            Authentication::AcceptLegalDocs => {}
            Authentication::GetTermsOfServiceConent => {}
            Authentication::CreateAccount => {}
            _ => {}
        }
        Components::GameManager(value) => {}
        Components::Redirector(value) => {}
        Components::Stats(value) => {}
        Components::Util(value) => {}
       _ => {}
    }

    Ok(())
}

packet! {
    struct Test {
        YES: String
    }
}

async fn test_route(_: Arc<Session>, packet: OpaquePacket) -> HandleResult {
    let test = packet.contents::<Test>()?;

    println!("{test:?}");
    Ok(())
}