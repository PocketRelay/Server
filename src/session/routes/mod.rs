use crate::utils::components::{self, Components as C};

use super::SessionLink;
use blaze_pk::router::Router;

mod auth;
mod game_manager;
mod messaging;
mod other;
mod stats;
mod user_sessions;
mod util;

/// Function which creates and sets up the router that directs incoming 
/// packets to different handling functions
/// 
/// rustfmt is disabled because it messes up the neat formatting of the 
/// route additions
#[rustfmt::skip]
pub fn router() -> Router<C, SessionLink> {
    let mut router = Router::new();

    // Authentication 
    {
        use auth::*;
        use components::Authentication as A;

        router.route(C::Authentication(A::Logout), handle_logout);
        router.route(C::Authentication(A::SilentLogin), handle_silent_login);
        router.route(C::Authentication(A::OriginLogin), handle_origin_login);
        router.route(C::Authentication(A::Login), handle_login);
        router.route(C::Authentication(A::LoginPersona), handle_login_persona);
        router.route(C::Authentication(A::ListUserEntitlements2), handle_list_entitlements);
        router.route(C::Authentication(A::CreateAccount),handle_create_account);
        router.route(C::Authentication(A::PasswordForgot), handle_forgot_password);
        router.route(C::Authentication(A::GetLegalDocsInfo), handle_get_legal_docs_info);
        router.route(C::Authentication(A::GetTermsOfServiceConent), || handle_legal_content(LegalType::TermsOfService));
        router.route(C::Authentication(A::GetPrivacyPolicyContent), || handle_legal_content(LegalType::PrivacyPolicy));
        router.route(C::Authentication(A::GetAuthToken),auth::handle_get_auth_token);
    }

    // Game Manager 
    {
        use game_manager::*;
        use components::GameManager as G;

        router.route(C::GameManager(G::CreateGame), handle_create_game);
        router.route(C::GameManager(G::AdvanceGameState), handle_set_state);
        router.route(C::GameManager(G::SetGameSettings), handle_set_setting);
        router.route(C::GameManager(G::SetGameAttributes), handle_set_attributes);
        router.route(C::GameManager(G::RemovePlayer), handle_remove_player);
        router.route(C::GameManager(G::RemovePlayer), handle_remove_player);
        router.route(C::GameManager(G::UpdateMeshConnection),handle_update_mesh_connection);
        router.route(C::GameManager(G::StartMatchmaking),handle_start_matchmaking);
        router.route(C::GameManager(G::CancelMatchmaking),handle_cancel_matchmaking);
        router.route(C::GameManager(G::GetGameDataFromID), handle_get_game_data);
        router.route(C::GameManager(G::JoinGame), handle_join_game);
    }

    // Stats
    {
        use stats::*;
        use components::Stats as S;

        router.route(C::Stats(S::GetLeaderboardEntityCount),handle_leaderboard_entity_count);
        router.route(C::Stats(S::GetLeaderboard), handle_normal_leaderboard);
        router.route(C::Stats(S::GetCenteredLeaderboard),handle_centered_leaderboard);
        router.route(C::Stats(S::GetFilteredLeaderboard),handle_filtered_leaderboard);
        router.route(C::Stats(S::GetLeaderboardGroup), handle_leaderboard_group);
    }

    // Util

    {
        use util::*;
        use components::Util as U;

        router.route(C::Util(U::PreAuth), handle_pre_auth);
        router.route(C::Util(U::PostAuth), handle_post_auth);
        router.route(C::Util(U::Ping), handle_ping);
        router.route(C::Util(U::FetchClientConfig), handle_fetch_client_config);
        router.route(C::Util(U::SuspendUserPing), handle_suspend_user_ping);
        router.route(C::Util(U::UserSettingsSave), handle_user_settings_save);
        router.route(C::Util(U::GetTelemetryServer), handle_get_telemetry_server);
        router.route(C::Util(U::GetTickerServer), handle_get_ticker_server);
        router.route(C::Util(U::UserSettingsLoadAll), handle_load_settings);
    }

    // Messaging
    {
        use messaging::*;
        use components::Messaging as M;

        router.route(C::Messaging(M::FetchMessages), handle_fetch_messages);
    }

    // User Sessions
    {
        use user_sessions::*;
        use components::UserSessions as U;

        router.route(C::UserSessions(U::ResumeSession), handle_resume_session);
        router.route(C::UserSessions(U::UpdateNetworkInfo), handle_update_network);
        router.route( C::UserSessions(U::UpdateHardwareFlags), handle_update_hardware_flag);
        router.route(C::UserSessions(U::LookupUser), handle_lookup_user);
    }

    // Game Reporting
    {
        use other::*;
        use components::GameReporting as G;

        router.route(C::GameReporting(G::SubmitOfflineGameReport),handle_submit_offline);
    }

    // Association Lists
    {
        use other::*;
        use components::AssociationLists as A;

        router.route(C::AssociationLists(A::GetLists), handle_get_lists);
    }

    router
}
