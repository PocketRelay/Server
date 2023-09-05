use crate::utils::components;

use super::router::Router;

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
pub fn router() -> Router {
    let mut router = Router::new();

    // Authentication 
    {
        use auth::*;
        use components::authentication as a;

        router.route(a::COMPONENT, a::LOGOUT, handle_logout);
        router.route(a::COMPONENT, a::SILENT_LOGIN, handle_silent_login);
        router.route(a::COMPONENT, a::ORIGIN_LOGIN, handle_origin_login);
        router.route(a::COMPONENT, a::LOGIN, handle_login);
        router.route(a::COMPONENT, a::LOGIN_PERSONA, handle_login_persona);
        router.route(a::COMPONENT, a::LIST_USER_ENTITLEMENTS_2, handle_list_entitlements);
        router.route(a::COMPONENT, a::CREATE_ACCOUNT,handle_create_account);
        router.route(a::COMPONENT, a::PASSWORD_FORGOT, handle_forgot_password);
        router.route(a::COMPONENT, a::GET_LEGAL_DOCS_INFO, handle_get_legal_docs_info);
        router.route(a::COMPONENT, a::GET_TERMS_OF_SERVICE_CONTENT, handle_tos);
        router.route(a::COMPONENT, a::GET_PRIVACY_POLICY_CONTENT, handle_privacy_policy);
        router.route(a::COMPONENT, a::GET_AUTH_TOKEN, handle_get_auth_token);
    }

    // Game Manager 
    {
        use game_manager::*;
        use components::game_manager as g;

        router.route(g::COMPONENT, g::CREATE_GAME, handle_create_game);
        router.route(g::COMPONENT, g::ADVANCE_GAME_STATE, handle_set_state);
        router.route(g::COMPONENT, g::SET_GAME_SETTINGS, handle_set_setting);
        router.route(g::COMPONENT, g::SET_GAME_ATTRIBUTES, handle_set_attributes);
        router.route(g::COMPONENT, g::REMOVE_PLAYER, handle_remove_player);
        router.route(g::COMPONENT, g::UPDATE_MESH_CONNECTION,handle_update_mesh_connection);
        router.route(g::COMPONENT, g::START_MATCHMAKING,handle_start_matchmaking);
        router.route(g::COMPONENT, g::CANCEL_MATCHMAKING,handle_cancel_matchmaking);
        router.route(g::COMPONENT, g::GET_GAME_DATA_FROM_ID, handle_get_game_data);
        router.route(g::COMPONENT, g::JOIN_GAME, handle_join_game);
    }

    // Stats
    {
        use stats::*;
        use components::stats as s;

        router.route(s::COMPONENT, s::GET_LEADERBOARD_ENTITY_COUNT, handle_leaderboard_entity_count);
        router.route(s::COMPONENT, s::GET_LEADERBOARD, handle_normal_leaderboard);
        router.route(s::COMPONENT, s::GET_CENTERED_LEADERBOARD,handle_centered_leaderboard);
        router.route(s::COMPONENT, s::GET_FILTERED_LEADERBOARD,handle_filtered_leaderboard);
        router.route(s::COMPONENT, s::GET_LEADERBOARD_GROUP, handle_leaderboard_group);
    }

    // Util

    {
        use util::*;
        use components::util as u;

        router.route(u::COMPONENT, u::PRE_AUTH, handle_pre_auth);
        router.route(u::COMPONENT, u::POST_AUTH, handle_post_auth);
        router.route(u::COMPONENT, u::PING, handle_ping);
        router.route(u::COMPONENT, u::FETCH_CLIENT_CONFIG, handle_fetch_client_config);
        router.route(u::COMPONENT, u::SUSPEND_USER_PING, handle_suspend_user_ping);
        router.route(u::COMPONENT, u::USER_SETTINGS_SAVE, handle_user_settings_save);
        router.route(u::COMPONENT, u::GET_TELEMETRY_SERVER, handle_get_telemetry_server);
        router.route(u::COMPONENT, u::GET_TICKER_SERVER, handle_get_ticker_server);
        router.route(u::COMPONENT, u::USER_SETTINGS_LOAD_ALL, handle_load_settings);
    }

    // Messaging
    {
        use messaging::*;
        use components::messaging as m;

        router.route(m::COMPONENT, m::FETCH_MESSAGES, handle_fetch_messages);
    }

    // User Sessions
    {
        use user_sessions::*;
        use components::user_sessions as u;

        router.route(u::COMPONENT, u::RESUME_SESSION, handle_resume_session);
        router.route(u::COMPONENT, u::UPDATE_NETWORK_INFO, handle_update_network);
        router.route(u::COMPONENT, u::UPDATE_HARDWARE_FLAGS, handle_update_hardware_flag);
        router.route(u::COMPONENT, u::LOOKUP_USER, handle_lookup_user);
    }

    // Game Reporting
    {
        use other::*;
        use components::game_reporting as g;

        router.route(g::COMPONENT, g::SUBMIT_OFFLINE_GAME_REPORT,handle_submit_offline);
    }

    // Association Lists
    {
        use other::*;
        use components::association_lists as a;

        router.route(a::COMPONENT, a::GET_LISTS, handle_get_lists);
    }

    router
}
