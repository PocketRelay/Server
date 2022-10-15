use std::sync::Arc;
use blaze_pk::OpaquePacket;
use crate::blaze::components::Authentication;
use crate::blaze::routes::HandleResult;
use crate::blaze::Session;

pub async fn route(_session: Arc<Session>, component: Authentication, _packet: OpaquePacket) -> HandleResult {
    match component {
        Authentication::UpdateAccount => {}
        Authentication::UpdateParentalEmail => {}
        Authentication::ListUserEntitlements2 => {}
        Authentication::GetAccount => {}
        Authentication::GrantEntitlement => {}
        Authentication::ListEntitlements => {}
        Authentication::HasEntitlement => {}
        Authentication::GetUseCount => {}
        Authentication::DecrementUseCount => {}
        Authentication::GetAuthToken => {}
        Authentication::GetHandoffToken => {}
        Authentication::GetPasswordRules => {}
        Authentication::GrantEntitlement2 => {}
        Authentication::Login => {}
        Authentication::AcceptTOS => {}
        Authentication::GetTOSInfo => {}
        Authentication::ModifyEntitlement2 => {}
        Authentication::ConsumeCode => {}
        Authentication::GetTOSContent => {}
        Authentication::GetPrivacyPolicyContent => {}
        Authentication::ListPersonalEntitlements2 => {}
        Authentication::SilentLogin => {}
        Authentication::CheckAgeRequirement => {}
        Authentication::GetOptIn => {}
        Authentication::EnableOptIn => {}
        Authentication::DisableOptIn => {}
        Authentication::ExpressLogin => {}
        Authentication::Logout => {}
        Authentication::CreatePersona => {}
        Authentication::GetPersona => {}
        Authentication::ListPersonas => {}
        Authentication::LoginPersona => {}
        Authentication::LogoutPersona => {}
        Authentication::DeletePersona => {}
        Authentication::DisablePersona => {}
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
        Authentication::DeviceLoginGuest => {}
        Authentication::CreateAccount => {}
        Authentication::Unknown(_) => {}
    }
    Ok(())
}