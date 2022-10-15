use std::sync::Arc;
use blaze_pk::OpaquePacket;
use crate::blaze::components::Util;
use crate::blaze::routes::HandleResult;
use crate::blaze::Session;

pub async fn route(_session: Arc<Session>, component: Util, _packet: OpaquePacket) -> HandleResult {
    match component {
        Util::FetchClientConfig => {}
        Util::Ping => {}
        Util::SetClientData => {}
        Util::LocalizeStrings => {}
        Util::GetTelemetryServer => {}
        Util::GetTickerServer => {}
        Util::PreAuth => {}
        Util::PostAuth => {}
        Util::UserSettingsLoad => {}
        Util::UserSettingsSave => {}
        Util::UserSettingsLoadAll => {}
        Util::DeleteUserSettings => {}
        Util::FilterForProfanity => {}
        Util::FetchQOSConfig => {}
        Util::SetClientMetrics => {}
        Util::SetConnectionState => {}
        Util::GetPSSConfig => {}
        Util::GetUserOptions => {}
        Util::SetUserOptions => {}
        Util::SuspendUserPing => {}
        Util::Unknown(_) => {}
    }
    Ok(())
}