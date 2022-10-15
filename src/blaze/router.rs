use std::sync::Arc;
use blaze_pk::{CodecError, OpaquePacket};
use derive_more::From;
use crate::blaze::components::{Authentication, Components};
use crate::blaze::Session;

#[derive(Debug, From)]
pub enum HandleError {
    CodecError(CodecError)
}

type HandleResult = Result<(), HandleError>;

pub async fn route(_: Arc<Session>, component: Components, packet: OpaquePacket) -> HandleResult {
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
        Components::GameManager(_) => {}
        Components::Redirector(_) => {}
        Components::Stats(_) => {}
        Components::Util(_) => {}
       _ => {}
    }

    Ok(())
}