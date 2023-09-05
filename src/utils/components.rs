use std::collections::HashMap;

static mut COMMANDS: Option<HashMap<u32, &'static str>> = None;
static mut NOTIFICATIONS: Option<HashMap<u32, &'static str>> = None;
static COMPONENT_NAMES: &[(u16, &str)] = &[
    (authentication::COMPONENT, "Authentication"),
    (game_manager::COMPONENT, "GameManager"),
    (redirector::COMPONENT, "Redirector"),
    (stats::COMPONENT, "Stats"),
    (util::COMPONENT, "Util"),
    (messaging::COMPONENT, "Messaging"),
    (association_lists::COMPONENT, "AssociationLists"),
    (game_reporting::COMPONENT, "GameReporting"),
    (user_sessions::COMPONENT, "UserSessions"),
];

/// Initializes the stored component state. Should only be
/// called on initial startup
pub fn initialize() {
    unsafe {
        COMMANDS = Some(commands());
        NOTIFICATIONS = Some(notifications())
    }
}

pub fn get_component_name(component: u16) -> Option<&'static str> {
    COMPONENT_NAMES
        .iter()
        .find_map(|(c, value)| if component.eq(c) { Some(value) } else { None })
        .copied()
}

pub fn get_command_name(component: u16, command: u16, notify: bool) -> Option<&'static str> {
    let key = debug_key(component, command);
    let map = if notify {
        unsafe { NOTIFICATIONS.as_ref() }
    } else {
        unsafe { COMMANDS.as_ref() }
    };
    map.and_then(|value| value.get(&key).copied())
}

/// Creates a key for looking up a command name
const fn debug_key(component: u16, command: u16) -> u32 {
    ((component as u32) << 16) + command as u32
}

pub mod authentication {
    pub const COMPONENT: u16 = 0x1;

    // Commands

    pub const CREATE_ACCOUNT: u16 = 0xA;
    pub const UPDATE_ACCOUNT: u16 = 0x14;
    pub const UPDATE_PARENTAL_EMAIL: u16 = 0x1C;
    pub const LIST_USER_ENTITLEMENTS_2: u16 = 0x1D;
    pub const GET_ACCOUNT: u16 = 0x1E;
    pub const GRANT_ENTITLEMENT: u16 = 0x1F;
    pub const LIST_ENTITLEMENTS: u16 = 0x20;
    pub const HAS_ENTITLEMENT: u16 = 0x21;
    pub const GET_USE_COUNT: u16 = 0x22;
    pub const DECREMENT_USE_COUNT: u16 = 0x23;
    pub const GET_AUTH_TOKEN: u16 = 0x24;
    pub const GET_HANDOFF_TOKEN: u16 = 0x25;
    pub const GET_PASSWORD_RULES: u16 = 0x26;
    pub const GRANT_ENTITLEMENT_2: u16 = 0x27;
    pub const LOGIN: u16 = 0x28;
    pub const ACCEPT_TOS: u16 = 0x29;
    pub const GET_TOS_INFO: u16 = 0x2A;
    pub const MODIFY_ENTITLEMENT_2: u16 = 0x2B;
    pub const CONSUME_CODE: u16 = 0x2C;
    pub const PASSWORD_FORGOT: u16 = 0x2D;
    pub const GET_TOS_CONTENT: u16 = 0x2E;
    pub const GET_PRIVACY_POLICY_CONTENT: u16 = 0x2F;
    pub const LIST_PERSONAL_ENTITLEMENTS_2: u16 = 0x30;
    pub const SILENT_LOGIN: u16 = 0x32;
    pub const CHECK_AGE_REQUIREMENT: u16 = 0x33;
    pub const GET_OPT_IN: u16 = 0x34;
    pub const ENABLE_OPT_IN: u16 = 0x35;
    pub const DISABLE_OPT_IN: u16 = 0x36;
    pub const EXPRESS_LOGIN: u16 = 0x3C;
    pub const LOGOUT: u16 = 0x46;
    pub const CREATE_PERSONA: u16 = 0x50;
    pub const GET_PERSONA: u16 = 0x5A;
    pub const LIST_PERSONAS: u16 = 0x64;
    pub const LOGIN_PERSONA: u16 = 0x6E;
    pub const LOGOUT_PERSONA: u16 = 0x78;
    pub const DELETE_PERSONA: u16 = 0x8C;
    pub const DISABLE_PERSONA: u16 = 0x8D;
    pub const LIST_DEVICE_ACCOUNTS: u16 = 0x8F;
    pub const XBOX_CREATE_ACCOUNT: u16 = 0x96;
    pub const ORIGIN_LOGIN: u16 = 0x98;
    pub const XBOX_ASSOCIATE_ACCOUNT: u16 = 0xA0;
    pub const XBOX_LOGIN: u16 = 0xAA;
    pub const PS3_CREATE_ACCOUNT: u16 = 0xB4;
    pub const PS3_ASSOCIATE_ACCOUNT: u16 = 0xBE;
    pub const PS3_LOGIN: u16 = 0xC8;
    pub const VALIDATE_SESSION_KEY: u16 = 0xD2;
    pub const CREATE_WAL_USER_SESSION: u16 = 0xE6;
    pub const ACCEPT_LEGAL_DOCS: u16 = 0xF1;
    pub const GET_LEGAL_DOCS_INFO: u16 = 0xF2;
    pub const GET_TERMS_OF_SERVICE_CONTENT: u16 = 0xF6;
    pub const DEVICE_LOGIN_GUEST: u16 = 0x12C;
}

pub mod game_manager {
    pub const COMPONENT: u16 = 0x4;

    // Components
    pub const CREATE_GAME: u16 = 0x1;
    pub const DESTROY_GAME: u16 = 0x2;
    pub const ADVANCE_GAME_STATE: u16 = 0x3;
    pub const SET_GAME_SETTINGS: u16 = 0x4;
    pub const SET_PLAYER_CAPACITY: u16 = 0x5;
    pub const SET_PRESENCE_MODE: u16 = 0x6;
    pub const SET_GAME_ATTRIBUTES: u16 = 0x7;
    pub const SET_PLAYER_ATTRIBUTES: u16 = 0x8;
    pub const JOIN_GAME: u16 = 0x9;
    // 0xA --
    pub const REMOVE_PLAYER: u16 = 0xB;
    // 0xC --
    pub const START_MATCHMAKING: u16 = 0xD;
    pub const CANCEL_MATCHMAKING: u16 = 0xE;
    pub const FINALIZE_GAME_CREATION: u16 = 0xF;
    // 0x10 --
    pub const LIST_GAMES: u16 = 0x11;
    pub const SET_PLAYER_CUSTOM_DATA: u16 = 0x12;
    pub const REPLAY_GAME: u16 = 0x13;
    pub const RETURN_DEDICATED_SERVER_TO_POOL: u16 = 0x14;
    pub const JOIN_GAME_BY_GROUP: u16 = 0x15;
    pub const LEAVE_GAME_BY_GROUP: u16 = 0x16;
    pub const MIGRATE_GAME: u16 = 0x17;
    pub const UPDATE_GAME_HOST_MIGRATION_STATUS: u16 = 0x18;
    pub const RESET_DEDICATED_SERVER: u16 = 0x19;
    pub const UPDATE_GAME_SESSION: u16 = 0x1A;
    pub const BAN_PLAYER: u16 = 0x1B;
    // 0x1C --
    pub const UPDATE_MESH_CONNECTION: u16 = 0x1D;
    // 0x1E --
    pub const REMOVE_PLAYER_FROM_BANNED_LIST: u16 = 0x1F;
    pub const CLEAR_BANNED_LIST: u16 = 0x20;
    pub const GET_BANNED_LIST: u16 = 0x21;
    // 0x22-0x25 --
    pub const ADD_QUEUED_PLAYER_TO_GAME: u16 = 0x26;
    pub const UPDATE_GAME_NAME: u16 = 0x27;
    pub const EJECT_HOST: u16 = 0x28;
    // 0x29-0x63 --
    pub const GET_GAME_LIST_SNAPSHOT: u16 = 0x64;
    pub const GET_GAME_LIST_SUBSCRIPTION: u16 = 0x65;
    pub const DESTROY_GAME_LIST: u16 = 0x66;
    pub const GET_FULL_GAME_DATA: u16 = 0x67;
    pub const GET_MATCHMAKING_CONFIG: u16 = 0x68;
    pub const GET_GAME_DATA_FROM_ID: u16 = 0x69;
    pub const ADD_ADMIN_PLAYER: u16 = 0x6A;
    pub const REMOVE_ADMIN_PLAYER: u16 = 0x6B;
    pub const SET_PLAYER_TEAM: u16 = 0x6C;
    pub const CHANGE_GAME_TEAM_ID: u16 = 0x6D;
    pub const MIGRATE_ADMIN_PLAYER: u16 = 0x6E;
    pub const GET_USER_SET_GAME_LIST_SUBSCRIPTION: u16 = 0x6F;
    pub const SWAP_PLAYERS_TEAM: u16 = 0x70;
    // 0x71-0x95 --
    pub const REGISTER_DYNAMIC_DEDICATED_SERVER_CREATOR: u16 = 0x96;
    pub const UNREGISTER_DYNAMIC_DEDICATED_SERVER_CREATOR: u16 = 0x97;

    // Notifications
    pub const MATCHMAKING_FAILED: u16 = 0xA;
    // 0xB --
    pub const MATCHMAKING_ASYNC_STATUS: u16 = 0xC;
    // 0xD-0xE --
    pub const GAME_CREATED: u16 = 0xF;
    pub const GAME_REMOVED: u16 = 0x10;
    // 0x11-0x13 --
    pub const GAME_SETUP: u16 = 0x14;
    pub const PLAYER_JOINING: u16 = 0x15;
    pub const JOINING_PLAYER_INITIATE_CONNECTIONS: u16 = 0x16;
    pub const PLAYER_JOINING_QUEUE: u16 = 0x17;
    pub const PLAYER_PROMOTED_FROM_QUEUE: u16 = 0x18;
    pub const PLAYER_CLAIMING_RESERVATION: u16 = 0x19;
    pub const PLAYER_JOIN_COMPLETED: u16 = 0x1E;
    // 0x1F-0x27 --
    pub const PLAYER_REMOVED: u16 = 0x28;
    // 0x29-0x3B --
    pub const HOST_MIGRATION_FINISHED: u16 = 0x3C;
    // 0x3D-0x45 --
    pub const HOST_MIGRATION_START: u16 = 0x46;
    pub const PLATFORM_HOST_INITIALIZED: u16 = 0x47;
    // 0x48-0x4F --
    pub const GAME_ATTRIB_CHANGE: u16 = 0x50;
    // 0x51-0x59 --
    pub const PLAYER_ATTRIB_CHANGE: u16 = 0x5A;
    pub const PLAYER_CUSTOM_DATA_CHANGE: u16 = 0x5F;
    // 0x60-0x63 --
    pub const GAME_STATE_CHANGE: u16 = 0x64;
    // 0x64-0x6D --
    pub const GAME_SETTINGS_CHANGE: u16 = 0x6E;
    pub const GAME_CAPACITY_CHANGE: u16 = 0x6F;
    pub const GAME_RESET: u16 = 0x70;
    pub const GAME_REPORTING_ID_CHANGE: u16 = 0x71;
    // 0x72 --
    pub const GAME_SESSION_UPDATED: u16 = 0x73;
    pub const GAME_PLAYER_STATE_CHANGE: u16 = 0x74;
    pub const GAME_PLAYER_TEAM_CHANGE: u16 = 0x75;
    pub const GAME_TEAM_ID_CHANGE: u16 = 0x76;
    pub const PROCESS_QUEUE: u16 = 0x77;
    pub const PRECENSE_MODE_CHANGED: u16 = 0x78;
    pub const GAME_PLAYER_QUEUE_POSITION_CHANGE: u16 = 0x79;
    // 0x7A-0xC8 --
    pub const GAME_LIST_UPDATE: u16 = 0xC9;
    pub const ADMIN_LIST_CHANGE: u16 = 0xCA;
    // 0xCB-0xDB --
    pub const CREATE_DYNAMIC_DEDICATED_SERVER_GAME: u16 = 0xDC;
    // 0xDD-0xE5 --
    pub const GAME_NAME_CHANGE: u16 = 0xE6;
}

pub mod redirector {
    pub const COMPONENT: u16 = 0x5;

    pub const GET_SERVER_INSTANCE: u16 = 0x1;
}

pub mod stats {
    pub const COMPONENT: u16 = 0x7;

    /// Components
    pub const GET_STAT_DECS: u16 = 0x1;
    pub const GET_STATS: u16 = 0x2;
    pub const GET_STAT_GROUP_LIST: u16 = 0x3;
    pub const GET_STAT_GROUP: u16 = 0x4;
    pub const GET_STATS_BY_GROUP: u16 = 0x5;
    pub const GET_DATE_RANGE: u16 = 0x6;
    pub const GET_ENTITY_COUNT: u16 = 0x7;
    // 0x8-0x9 --
    pub const GET_LEADERBOARD_GROUP: u16 = 0xA;
    pub const GET_LEADERBOARD_FOLDER_GROUP: u16 = 0xB;
    pub const GET_LEADERBOARD: u16 = 0xC;
    pub const GET_CENTERED_LEADERBOARD: u16 = 0xD;
    pub const GET_FILTERED_LEADERBOARD: u16 = 0xE;
    pub const GET_KEY_SCOPES_MAP: u16 = 0xF;
    pub const GET_STATS_BY_GROUP_ASYNC: u16 = 0x10;
    pub const GET_LEADERBOARD_TREE_ASYNC: u16 = 0x11;
    pub const GET_LEADERBOARD_ENTITY_COUNT: u16 = 0x12;
    pub const GET_STAT_CATEGORY_LIST: u16 = 0x13;
    pub const GET_PERIOD_IDS: u16 = 0x14;
    pub const GET_LEADERBOARD_RAW: u16 = 0x15;
    pub const GET_CENTERED_LEADERBOARD_RAW: u16 = 0x16;
    pub const GET_FILTERED_LEADERBOARD_RAW: u16 = 0x17;
    pub const CHANGE_KEY_SCOPE_VALUE: u16 = 0x18;
}

pub mod util {
    pub const COMPONENT: u16 = 0x9;

    /// Components
    pub const FETCH_CLIENT_CONFIG: u16 = 0x1;
    pub const PING: u16 = 0x2;
    pub const SET_CLIENT_DATA: u16 = 0x3;
    pub const LOCALIZE_STRINGS: u16 = 0x4;
    pub const GET_TELEMETRY_SERVER: u16 = 0x5;
    pub const GET_TICKER_SERVER: u16 = 0x6;
    pub const PRE_AUTH: u16 = 0x7;
    pub const POST_AUTH: u16 = 0x8;
    // 0x9 --
    pub const USER_SETTINGS_LOAD: u16 = 0xA;
    pub const USER_SETTINGS_SAVE: u16 = 0xB;
    pub const USER_SETTINGS_LOAD_ALL: u16 = 0xC;
    // 0xD --
    pub const DELETE_USER_SETTINGS: u16 = 0xE;
    //
    pub const FILTER_FOR_PROFANITY: u16 = 0x14;
    pub const FETCH_QOS_CONFIG: u16 = 0x15;
    pub const SET_CLIENT_METRICS: u16 = 0x16;
    pub const SET_CONNECTION_STATE: u16 = 0x17;
    pub const GET_PSS_CONFIG: u16 = 0x18;
    pub const GET_USER_OPTIONS: u16 = 0x19;
    pub const SET_USER_OPTIONS: u16 = 0x1A;
    pub const SUSPEND_USER_PING: u16 = 0x1B;
}

pub mod messaging {
    pub const COMPONENT: u16 = 0xF;

    /// Components
    pub const FETCH_MESSAGES: u16 = 0x2;
    pub const PURGE_MESSAGES: u16 = 0x3;
    pub const TOUCH_MESSAGES: u16 = 0x4;
    pub const GET_MESSAGES: u16 = 0x5;

    // Notifications
    pub const SEND_MESSAGE: u16 = 0x1;
}

pub mod association_lists {
    pub const COMPONENT: u16 = 0x19;

    // Components
    pub const ADD_USERS_TO_LIST: u16 = 0x1;
    pub const REMOVE_USERS_FROM_LIST: u16 = 0x2;
    pub const CLEAR_LIST: u16 = 0x3;
    pub const SET_USERS_TO_LIST: u16 = 0x4;
    pub const GET_LIST_FOR_USER: u16 = 0x5;
    pub const GET_LISTS: u16 = 0x6;
    pub const SUBSCRIBE_TO_LISTS: u16 = 0x7;
    pub const UNSUBSCRIBE_TO_LISTS: u16 = 0x8;
    pub const GET_CONFIG_LISTS_INFO: u16 = 0x9;
}

pub mod game_reporting {
    pub const COMPONENT: u16 = 0x1C;

    // Components
    pub const SUBMIT_GAME_REPORT: u16 = 0x1;
    pub const SUBMIT_OFFLINE_GAME_REPORT: u16 = 0x2;
    pub const SUBMIT_GAME_EVENTS: u16 = 0x3;
    pub const GET_GAME_REPORT_QUERY: u16 = 0x4;
    pub const GET_GAME_REPORT_QUERIES_LIST: u16 = 0x5;
    pub const GET_GAME_REPORTS: u16 = 0x6;
    pub const GET_GAME_REPORT_VIEW: u16 = 0x7;
    pub const GET_GAME_REPORT_VIEW_INFO: u16 = 0x8;
    pub const GET_GAME_REPORT_VIEW_INFO_LIST: u16 = 0x9;
    pub const GET_GAME_REPORT_TYPES: u16 = 0xA;
    pub const UPDATE_METRICS: u16 = 0xB;
    pub const GET_GAME_REPORT_COLUMN_INFO: u16 = 0xC;
    pub const GET_GAME_REPORT_COLUMN_VALUES: u16 = 0xD;
    // 0xE-0x63 --
    pub const SUBMIT_TRUSTED_MID_GAME_REPORT: u16 = 0x64;
    pub const SUBMIT_TRUSTED_END_GAME_REPORT: u16 = 0x65;

    // Notifications
    pub const GAME_REPORT_SUBMITTED: u16 = 0x72;
}

pub mod user_sessions {
    pub const COMPONENT: u16 = 0x7802;

    // Components
    pub const UPDATE_HARDWARE_FLAGS: u16 = 0x8;
    pub const LOOKUP_USER: u16 = 0xC;
    pub const LOOKUP_USERS: u16 = 0xD;
    pub const LOOKUP_USERS_BY_PREFIX: u16 = 0xE;
    // 0xF-0x13 --
    pub const UPDATE_NETWORK_INFO: u16 = 0x14;
    // 0x15-0x16 --
    pub const LOOKUP_USER_GEO_IP_DATA: u16 = 0x17;
    pub const OVERRIDE_USER_GEO_IP_DATA: u16 = 0x18;
    pub const UPDATE_USER_SESSION_CLIENT_DATA: u16 = 0x19;
    pub const SET_USER_INFO_ATTRIBUTE: u16 = 0x1A;
    pub const RESET_USER_GEO_IP_DATA: u16 = 0x1B;
    // 0x1C-0x1F --
    pub const LOOKUP_USER_SESSION_ID: u16 = 0x20;
    pub const FETCH_LAST_LOCALE_USED_AND_AUTH_ERROR: u16 = 0x21;
    pub const FETCH_USER_FIRST_LAST_AUTH_TIME: u16 = 0x22;
    pub const RESUME_SESSION: u16 = 0x23;

    // Notifications
    pub const SET_SESSION: u16 = 0x1;
    pub const SESSION_DETAILS: u16 = 0x2;
    pub const FETCH_EXTENDED_DATA: u16 = 0x3;
    pub const UPDATE_EXTENDED_DATA_ATTRIBUTE: u16 = 0x5;
}

#[rustfmt::skip]
fn commands() -> HashMap<u32, &'static str> {
    use authentication as a;
    use game_manager as g;
    use redirector as r;
    use stats as s;
    use util as u;
    use messaging as m;
    use association_lists as al;
    use game_reporting as gr;
    use user_sessions as us;

    [
        // Authentication
        (debug_key(a::COMPONENT, a::CREATE_ACCOUNT), "CreateAccount"),
        (debug_key(a::COMPONENT, a::UPDATE_ACCOUNT), "UpdateAccount"),
        (debug_key(a::COMPONENT, a::UPDATE_PARENTAL_EMAIL), "UpdateParentalEmail"),
        (debug_key(a::COMPONENT, a::LIST_USER_ENTITLEMENTS_2), "ListUserEntitlements2"),
        (debug_key(a::COMPONENT, a::GET_ACCOUNT), "GetAccount"),
        (debug_key(a::COMPONENT, a::GRANT_ENTITLEMENT), "GrantEntitlement"),
        (debug_key(a::COMPONENT, a::LIST_ENTITLEMENTS), "ListEntitlements"),
        (debug_key(a::COMPONENT, a::HAS_ENTITLEMENT), "HasEntitlement"),
        (debug_key(a::COMPONENT, a::GET_USE_COUNT), "GetUseCount"),
        (debug_key(a::COMPONENT, a::DECREMENT_USE_COUNT), "DecrementUseCount"),
        (debug_key(a::COMPONENT, a::GET_AUTH_TOKEN), "GetAuthToken"),
        (debug_key(a::COMPONENT, a::GET_HANDOFF_TOKEN), "GetHandoffToken"),
        (debug_key(a::COMPONENT, a::GET_PASSWORD_RULES), "GetPasswordRules"),
        (debug_key(a::COMPONENT, a::GRANT_ENTITLEMENT_2), "GrantEntitlement2"),
        (debug_key(a::COMPONENT, a::LOGIN), "Login"),
        (debug_key(a::COMPONENT, a::ACCEPT_TOS), "AcceptTOS"),
        (debug_key(a::COMPONENT, a::GET_TOS_INFO), "GetTOSInfo"),
        (debug_key(a::COMPONENT, a::MODIFY_ENTITLEMENT_2), "ModifyEntitlement2"),
        (debug_key(a::COMPONENT, a::CONSUME_CODE), "ConsumeCode"),
        (debug_key(a::COMPONENT, a::PASSWORD_FORGOT), "PasswordForgot"),
        (debug_key(a::COMPONENT, a::GET_TOS_CONTENT), "GetTOSContent"),
        (debug_key(a::COMPONENT, a::GET_PRIVACY_POLICY_CONTENT), "GetPrivacyPolicyContent"),
        (debug_key(a::COMPONENT, a::LIST_PERSONAL_ENTITLEMENTS_2), "ListPersonalEntitlements2"),
        (debug_key(a::COMPONENT, a::SILENT_LOGIN), "SilentLogin"),
        (debug_key(a::COMPONENT, a::CHECK_AGE_REQUIREMENT), "CheckAgeRequirement"),
        (debug_key(a::COMPONENT, a::GET_OPT_IN), "GetOptIn"),
        (debug_key(a::COMPONENT, a::ENABLE_OPT_IN), "EnableOptIn"),
        (debug_key(a::COMPONENT, a::DISABLE_OPT_IN), "DisableOptIn"),
        (debug_key(a::COMPONENT, a::EXPRESS_LOGIN), "ExpressLogin"),
        (debug_key(a::COMPONENT, a::LOGOUT), "Logout"),
        (debug_key(a::COMPONENT, a::CREATE_PERSONA), "CreatePersona"),
        (debug_key(a::COMPONENT, a::GET_PERSONA), "GetPersona"),
        (debug_key(a::COMPONENT, a::LIST_PERSONAS), "ListPersonas"),
        (debug_key(a::COMPONENT, a::LOGIN_PERSONA), "LoginPersona"),
        (debug_key(a::COMPONENT, a::LOGOUT_PERSONA), "LogoutPersona"),
        (debug_key(a::COMPONENT, a::DELETE_PERSONA), "DeletePersona"),
        (debug_key(a::COMPONENT, a::DISABLE_PERSONA), "DisablePersona"),
        (debug_key(a::COMPONENT, a::LIST_DEVICE_ACCOUNTS), "ListDeviceAccounts"),
        (debug_key(a::COMPONENT, a::XBOX_CREATE_ACCOUNT), "XboxCreateAccount"),
        (debug_key(a::COMPONENT, a::ORIGIN_LOGIN), "OriginLogin"),
        (debug_key(a::COMPONENT, a::XBOX_ASSOCIATE_ACCOUNT), "XboxAssociateAccount"),
        (debug_key(a::COMPONENT, a::XBOX_LOGIN), "XboxLogin"),
        (debug_key(a::COMPONENT, a::PS3_CREATE_ACCOUNT), "PS3CreateAccount"),
        (debug_key(a::COMPONENT, a::PS3_ASSOCIATE_ACCOUNT), "PS3AssociateAccount"),
        (debug_key(a::COMPONENT, a::PS3_LOGIN), "PS3Login"),
        (debug_key(a::COMPONENT, a::VALIDATE_SESSION_KEY), "ValidateSessionKey"),
        (debug_key(a::COMPONENT, a::CREATE_WAL_USER_SESSION), "CreateWalUserSession"),
        (debug_key(a::COMPONENT, a::ACCEPT_LEGAL_DOCS), "AcceptLegalDocs"),
        (debug_key(a::COMPONENT, a::GET_LEGAL_DOCS_INFO), "GetLegalDocsInfo"),
        (debug_key(a::COMPONENT, a::GET_TERMS_OF_SERVICE_CONTENT), "GetTermsOfServiceContent"),
        (debug_key(a::COMPONENT, a::DEVICE_LOGIN_GUEST), "DeviceLoginGuest"),
        
        // Game Manager
        (debug_key(g::COMPONENT, g::CREATE_GAME), "CreateGame"),
        (debug_key(g::COMPONENT, g::DESTROY_GAME), "DestroyGame"),
        (debug_key(g::COMPONENT, g::ADVANCE_GAME_STATE), "AdvanceGameState"),
        (debug_key(g::COMPONENT, g::SET_GAME_SETTINGS), "SetGameSettings"),
        (debug_key(g::COMPONENT, g::SET_PLAYER_CAPACITY), "SetPlayerCapacity"),
        (debug_key(g::COMPONENT, g::SET_PRESENCE_MODE), "SetPresenceMode"),
        (debug_key(g::COMPONENT, g::SET_GAME_ATTRIBUTES), "SetGameAttributes"),
        (debug_key(g::COMPONENT, g::SET_PLAYER_ATTRIBUTES), "SetPlayerAttributes"),
        (debug_key(g::COMPONENT, g::JOIN_GAME), "JoinGame"),
        (debug_key(g::COMPONENT, g::REMOVE_PLAYER), "RemovePlayer"),
        (debug_key(g::COMPONENT, g::START_MATCHMAKING), "StartMatchmaking"),
        (debug_key(g::COMPONENT, g::CANCEL_MATCHMAKING), "CancelMatchmaking"),
        (debug_key(g::COMPONENT, g::FINALIZE_GAME_CREATION), "FinalizeGameCreation"),
        (debug_key(g::COMPONENT, g::LIST_GAMES), "ListGames"),
        (debug_key(g::COMPONENT, g::SET_PLAYER_CUSTOM_DATA), "SetPlayerCustomData"),
        (debug_key(g::COMPONENT, g::REPLAY_GAME), "ReplayGame"),
        (debug_key(g::COMPONENT, g::RETURN_DEDICATED_SERVER_TO_POOL), "ReturnDedicatedServerToPool"),
        (debug_key(g::COMPONENT, g::JOIN_GAME_BY_GROUP), "JoinGameByGroup"),
        (debug_key(g::COMPONENT, g::LEAVE_GAME_BY_GROUP), "LeaveGameByGroup"),
        (debug_key(g::COMPONENT, g::MIGRATE_GAME), "MigrateGame"),
        (debug_key(g::COMPONENT, g::UPDATE_GAME_HOST_MIGRATION_STATUS), "UpdateGameHostMigrationStatus"),
        (debug_key(g::COMPONENT, g::RESET_DEDICATED_SERVER), "ResetDedicatedServer"),
        (debug_key(g::COMPONENT, g::UPDATE_GAME_SESSION), "UpdateGameSession"),
        (debug_key(g::COMPONENT, g::BAN_PLAYER), "BanPlayer"),
        (debug_key(g::COMPONENT, g::UPDATE_MESH_CONNECTION), "UpdateMeshConnection"),
        (debug_key(g::COMPONENT, g::REMOVE_PLAYER_FROM_BANNED_LIST), "RemovePlayerFromBannedList"),
        (debug_key(g::COMPONENT, g::CLEAR_BANNED_LIST), "ClearBannedList"),
        (debug_key(g::COMPONENT, g::GET_BANNED_LIST), "GetBannedList"),
        (debug_key(g::COMPONENT, g::ADD_QUEUED_PLAYER_TO_GAME), "AddQueuedPlayerToGame"),
        (debug_key(g::COMPONENT, g::UPDATE_GAME_NAME), "UpdateGameName"),
        (debug_key(g::COMPONENT, g::EJECT_HOST), "EjectHost"),
        (debug_key(g::COMPONENT, g::GET_GAME_LIST_SNAPSHOT), "GetGameListSnapshot"),
        (debug_key(g::COMPONENT, g::GET_GAME_LIST_SUBSCRIPTION), "GetGameListSubscription"),
        (debug_key(g::COMPONENT, g::DESTROY_GAME_LIST), "DestroyGameList"),
        (debug_key(g::COMPONENT, g::GET_FULL_GAME_DATA), "GetFullGameData"),
        (debug_key(g::COMPONENT, g::GET_MATCHMAKING_CONFIG), "GetMatchmakingConfig"),
        (debug_key(g::COMPONENT, g::GET_GAME_DATA_FROM_ID), "GetGameDataFromID"),
        (debug_key(g::COMPONENT, g::ADD_ADMIN_PLAYER), "AddAdminPlayer"),
        (debug_key(g::COMPONENT, g::REMOVE_ADMIN_PLAYER), "RemoveAdminPlayer"),
        (debug_key(g::COMPONENT, g::SET_PLAYER_TEAM), "SetPlayerTeam"),
        (debug_key(g::COMPONENT, g::CHANGE_GAME_TEAM_ID), "ChangeGameTeamID"),
        (debug_key(g::COMPONENT, g::MIGRATE_ADMIN_PLAYER), "MigrateAdminPlayer"),
        (debug_key(g::COMPONENT, g::GET_USER_SET_GAME_LIST_SUBSCRIPTION), "GetUserSetGameListSubscription"),
        (debug_key(g::COMPONENT, g::SWAP_PLAYERS_TEAM), "SwapPlayersTeam"),
        (debug_key(g::COMPONENT, g::REGISTER_DYNAMIC_DEDICATED_SERVER_CREATOR), "RegisterDynamicDedicatedServerCreator"),
        (debug_key(g::COMPONENT, g::UNREGISTER_DYNAMIC_DEDICATED_SERVER_CREATOR), "UnregisterDynamicDedicatedServerCreator"),
       
        // Redirector  
        (debug_key(r::COMPONENT, r::GET_SERVER_INSTANCE), "GetServerInstance"),
        
        // Stats
        (debug_key(s::COMPONENT, s::GET_STAT_DECS), "GetStatDecs"),
        (debug_key(s::COMPONENT, s::GET_STATS), "GetStats"),
        (debug_key(s::COMPONENT, s::GET_STAT_GROUP_LIST), "GetStatGroupList"),
        (debug_key(s::COMPONENT, s::GET_STAT_GROUP), "GetStatGroup"),
        (debug_key(s::COMPONENT, s::GET_STATS_BY_GROUP), "GetStatsByGroup"),
        (debug_key(s::COMPONENT, s::GET_DATE_RANGE), "GetDateRange"),
        (debug_key(s::COMPONENT, s::GET_ENTITY_COUNT), "GetEntityCount"),
        (debug_key(s::COMPONENT, s::GET_LEADERBOARD_GROUP), "GetLeaderboardGroup"),
        (debug_key(s::COMPONENT, s::GET_LEADERBOARD_FOLDER_GROUP), "GetLeaderboardFolderGroup"),
        (debug_key(s::COMPONENT, s::GET_LEADERBOARD), "GetLeaderboard"),
        (debug_key(s::COMPONENT, s::GET_CENTERED_LEADERBOARD), "GetCenteredLeaderboard"),
        (debug_key(s::COMPONENT, s::GET_FILTERED_LEADERBOARD), "GetFilteredLeaderboard"),
        (debug_key(s::COMPONENT, s::GET_KEY_SCOPES_MAP), "GetKeyScopesMap"),
        (debug_key(s::COMPONENT, s::GET_STATS_BY_GROUP_ASYNC), "GetStatsByGroupASync"),
        (debug_key(s::COMPONENT, s::GET_LEADERBOARD_TREE_ASYNC), "GetLeaderboardTreeAsync"),
        (debug_key(s::COMPONENT, s::GET_LEADERBOARD_ENTITY_COUNT), "GetLeaderboardEntityCount"),
        (debug_key(s::COMPONENT, s::GET_STAT_CATEGORY_LIST), "GetStatCategoryList"),
        (debug_key(s::COMPONENT, s::GET_PERIOD_IDS), "GetPeriodIDs"),
        (debug_key(s::COMPONENT, s::GET_LEADERBOARD_RAW), "GetLeaderboardRaw"),
        (debug_key(s::COMPONENT, s::GET_CENTERED_LEADERBOARD_RAW), "GetCenteredLeaderboardRaw"),
        (debug_key(s::COMPONENT, s::GET_FILTERED_LEADERBOARD_RAW), "GetFilteredLeaderboardRaw"),
        (debug_key(s::COMPONENT, s::CHANGE_KEY_SCOPE_VALUE), "ChangeKeyScopeValue"),
        
        // Util
        (debug_key(u::COMPONENT, u::FETCH_CLIENT_CONFIG), "FetchClientConfig"),
        (debug_key(u::COMPONENT, u::PING), "Ping"),
        (debug_key(u::COMPONENT, u::SET_CLIENT_DATA), "SetClientData"),
        (debug_key(u::COMPONENT, u::LOCALIZE_STRINGS), "LocalizeStrings"),
        (debug_key(u::COMPONENT, u::GET_TELEMETRY_SERVER), "GetTelemetryServer"),
        (debug_key(u::COMPONENT, u::GET_TICKER_SERVER), "GetTickerServer"),
        (debug_key(u::COMPONENT, u::PRE_AUTH), "PreAuth"),
        (debug_key(u::COMPONENT, u::POST_AUTH), "PostAuth"),
        (debug_key(u::COMPONENT, u::USER_SETTINGS_LOAD), "UserSettingsLoad"),
        (debug_key(u::COMPONENT, u::USER_SETTINGS_SAVE), "UserSettingsSave"),
        (debug_key(u::COMPONENT, u::USER_SETTINGS_LOAD_ALL), "UserSettingsLoadAll"),
        (debug_key(u::COMPONENT, u::DELETE_USER_SETTINGS), "DeleteUserSettings"),
        (debug_key(u::COMPONENT, u::FILTER_FOR_PROFANITY), "FilterForProfanity"),
        (debug_key(u::COMPONENT, u::FETCH_QOS_CONFIG), "FetchQOSConfig"),
        (debug_key(u::COMPONENT, u::SET_CLIENT_METRICS), "SetClientMetrics"),
        (debug_key(u::COMPONENT, u::SET_CONNECTION_STATE), "SetConnectionState"),
        (debug_key(u::COMPONENT, u::GET_PSS_CONFIG), "GetPSSConfig"),
        (debug_key(u::COMPONENT, u::GET_USER_OPTIONS), "GetUserOptions"),
        (debug_key(u::COMPONENT, u::SET_USER_OPTIONS), "SetUserOptions"),
        (debug_key(u::COMPONENT, u::SUSPEND_USER_PING), "SuspendUserPing"),
        
        // Messaging
        (debug_key(m::COMPONENT, m::FETCH_MESSAGES), "FetchMessages"),
        (debug_key(m::COMPONENT, m::PURGE_MESSAGES), "PurgeMessages"),
        (debug_key(m::COMPONENT, m::TOUCH_MESSAGES), "TouchMessages"),
        (debug_key(m::COMPONENT, m::GET_MESSAGES), "GetMessages"),
        
        // Association Lists
        (debug_key(al::COMPONENT, al::ADD_USERS_TO_LIST), "AddUsersToList"),
        (debug_key(al::COMPONENT, al::REMOVE_USERS_FROM_LIST), "RemoveUsersFromList"),
        (debug_key(al::COMPONENT, al::CLEAR_LIST), "ClearList"),
        (debug_key(al::COMPONENT, al::SET_USERS_TO_LIST), "SetUsersToList"),
        (debug_key(al::COMPONENT, al::GET_LIST_FOR_USER), "GetListForUser"),
        (debug_key(al::COMPONENT, al::GET_LISTS), "GetLists"),
        (debug_key(al::COMPONENT, al::SUBSCRIBE_TO_LISTS), "SubscribeToLists"),
        (debug_key(al::COMPONENT, al::UNSUBSCRIBE_TO_LISTS), "UnsubscribeToLists"),
        (debug_key(al::COMPONENT, al::GET_CONFIG_LISTS_INFO), "GetConfigListsInfo"),

        // Game Reporting
        (debug_key(gr::COMPONENT, gr::SUBMIT_GAME_REPORT), "SubmitGameReport"),
        (debug_key(gr::COMPONENT, gr::SUBMIT_OFFLINE_GAME_REPORT), "SubmitOfflineGameReport"),
        (debug_key(gr::COMPONENT, gr::SUBMIT_GAME_EVENTS), "SubmitGameEvents"),
        (debug_key(gr::COMPONENT, gr::GET_GAME_REPORT_QUERY), "GetGameReportQuery"),
        (debug_key(gr::COMPONENT, gr::GET_GAME_REPORT_QUERIES_LIST), "GetGameReportQueriesList"),
        (debug_key(gr::COMPONENT, gr::GET_GAME_REPORTS), "GetGameReports"),
        (debug_key(gr::COMPONENT, gr::GET_GAME_REPORT_VIEW), "GetGameReportView"),
        (debug_key(gr::COMPONENT, gr::GET_GAME_REPORT_VIEW_INFO), "GetGameReportViewInfo"),
        (debug_key(gr::COMPONENT, gr::GET_GAME_REPORT_VIEW_INFO_LIST), "GetGameReportViewInfoList"),
        (debug_key(gr::COMPONENT, gr::GET_GAME_REPORT_TYPES), "GetGameReportTypes"),
        (debug_key(gr::COMPONENT, gr::UPDATE_METRICS), "UpdateMetrics"),
        (debug_key(gr::COMPONENT, gr::GET_GAME_REPORT_COLUMN_INFO), "GetGameReportColumnInfo"),
        (debug_key(gr::COMPONENT, gr::GET_GAME_REPORT_COLUMN_VALUES), "GetGameReportColumnValues"),
        (debug_key(gr::COMPONENT, gr::SUBMIT_TRUSTED_MID_GAME_REPORT), "SubmitTrustedMidGameReport"),
        (debug_key(gr::COMPONENT, gr::SUBMIT_TRUSTED_END_GAME_REPORT), "SubmitTrustedEndGameReport"),
     
        // User Sessions
        (debug_key(us::COMPONENT, us::UPDATE_HARDWARE_FLAGS), "UpdateHardwareFlags"),
        (debug_key(us::COMPONENT, us::LOOKUP_USER), "LookupUser"),
        (debug_key(us::COMPONENT, us::LOOKUP_USERS), "LookupUsers"),
        (debug_key(us::COMPONENT, us::LOOKUP_USERS_BY_PREFIX), "LookupUsersByPrefix"),
        (debug_key(us::COMPONENT, us::UPDATE_NETWORK_INFO), "UpdateNetworkInfo"),
        (debug_key(us::COMPONENT, us::LOOKUP_USER_GEO_IP_DATA), "LookupUserGeoIPData"),
        (debug_key(us::COMPONENT, us::OVERRIDE_USER_GEO_IP_DATA), "OverrideUserGeoIPData"),
        (debug_key(us::COMPONENT, us::UPDATE_USER_SESSION_CLIENT_DATA), "UpdateUserSessionClientData"),
        (debug_key(us::COMPONENT, us::SET_USER_INFO_ATTRIBUTE), "SetUserInfoAttribute"),
        (debug_key(us::COMPONENT, us::RESET_USER_GEO_IP_DATA), "ResetUserGeoIPData"),
        (debug_key(us::COMPONENT, us::LOOKUP_USER_SESSION_ID), "LookupUserSessionID"),
        (debug_key(us::COMPONENT, us::FETCH_LAST_LOCALE_USED_AND_AUTH_ERROR), "FetchLastLocaleUsedAndAuthError"),
        (debug_key(us::COMPONENT, us::FETCH_USER_FIRST_LAST_AUTH_TIME), "FetchUserFirstLastAuthTime"),
        (debug_key(us::COMPONENT, us::RESUME_SESSION), "ResumeSession"),
    ]
    .into_iter()
    .collect()
}

#[rustfmt::skip]
fn notifications() -> HashMap<u32, &'static str> {
    use game_manager as g;
    use messaging as m;
    use game_reporting as gr;
    use user_sessions as us;

    [
        // Game Manager
        (debug_key(g::COMPONENT, g::MATCHMAKING_FAILED), "MatchmakingFailed"),
        (debug_key(g::COMPONENT, g::MATCHMAKING_ASYNC_STATUS), "MatchmakingAsyncStatus"),
        (debug_key(g::COMPONENT, g::GAME_CREATED), "GameCreated"),
        (debug_key(g::COMPONENT, g::GAME_REMOVED), "GameRemoved"),
        (debug_key(g::COMPONENT, g::GAME_SETUP), "GameSetup"),
        (debug_key(g::COMPONENT, g::PLAYER_JOINING), "PlayerJoining"),
        (debug_key(g::COMPONENT, g::JOINING_PLAYER_INITIATE_CONNECTIONS), "JoiningPlayerInitiateConnections"),
        (debug_key(g::COMPONENT, g::PLAYER_JOINING_QUEUE), "PlayerJoiningQueue"),
        (debug_key(g::COMPONENT, g::PLAYER_PROMOTED_FROM_QUEUE), "PlayerPromotedFromQueue"),
        (debug_key(g::COMPONENT, g::PLAYER_CLAIMING_RESERVATION), "PlayerClaimingReservation"),
        (debug_key(g::COMPONENT, g::PLAYER_JOIN_COMPLETED), "PlayerJoinCompleted"),
        (debug_key(g::COMPONENT, g::PLAYER_REMOVED), "PlayerRemoved"),
        (debug_key(g::COMPONENT, g::HOST_MIGRATION_FINISHED), "HostMigrationFinished"),
        (debug_key(g::COMPONENT, g::HOST_MIGRATION_START), "HostMigrationStart"),
        (debug_key(g::COMPONENT, g::PLATFORM_HOST_INITIALIZED), "PlatformHostInitialized"),
        (debug_key(g::COMPONENT, g::GAME_ATTRIB_CHANGE), "GameAttribChange"),
        (debug_key(g::COMPONENT, g::PLAYER_ATTRIB_CHANGE), "PlayerAttribChange"),
        (debug_key(g::COMPONENT, g::PLAYER_CUSTOM_DATA_CHANGE), "PlayerCustomDataChange"),
        (debug_key(g::COMPONENT, g::GAME_STATE_CHANGE), "GameStateChange"),
        (debug_key(g::COMPONENT, g::GAME_SETTINGS_CHANGE), "GameSettingsChange"),
        (debug_key(g::COMPONENT, g::GAME_CAPACITY_CHANGE), "GameCapacityChange"),
        (debug_key(g::COMPONENT, g::GAME_RESET), "GameReset"),
        (debug_key(g::COMPONENT, g::GAME_REPORTING_ID_CHANGE), "GameReportingIDChange"),
        (debug_key(g::COMPONENT, g::GAME_SESSION_UPDATED), "GameSessionUpdated"),
        (debug_key(g::COMPONENT, g::GAME_PLAYER_STATE_CHANGE), "GamePlayerStateChange"),
        (debug_key(g::COMPONENT, g::GAME_PLAYER_TEAM_CHANGE), "GamePlayerTeamChange"),
        (debug_key(g::COMPONENT, g::GAME_TEAM_ID_CHANGE), "GameTeamIDChange"),
        (debug_key(g::COMPONENT, g::PROCESS_QUEUE), "PROCESS_QUEUE"),
        (debug_key(g::COMPONENT, g::PRECENSE_MODE_CHANGED), "PrecenseModeChanged"),
        (debug_key(g::COMPONENT, g::GAME_PLAYER_QUEUE_POSITION_CHANGE), "GamePlayerQueuePositionChange"),
        (debug_key(g::COMPONENT, g::GAME_LIST_UPDATE), "GameListUpdate"),
        (debug_key(g::COMPONENT, g::ADMIN_LIST_CHANGE), "AdminListChange"),
        (debug_key(g::COMPONENT, g::CREATE_DYNAMIC_DEDICATED_SERVER_GAME), "CreateDynamicDedicatedServerGame"),
        (debug_key(g::COMPONENT, g::GAME_NAME_CHANGE), "GameNameChange"),

        // Messaging
        (debug_key(m::COMPONENT, m::SEND_MESSAGE), "SendMessage"),

        // Game Reporting
        (debug_key(gr::COMPONENT, gr::GAME_REPORT_SUBMITTED), "GameReportSubmitted"),

        // User Sessions
        (debug_key(us::COMPONENT, us::SET_SESSION), "SetSession"),
        (debug_key(us::COMPONENT, us::SESSION_DETAILS), "SessionDetails"),
        (debug_key(us::COMPONENT, us::UPDATE_EXTENDED_DATA_ATTRIBUTE), "UpdateExtendedDataAttribute"),
        (debug_key(us::COMPONENT, us::FETCH_EXTENDED_DATA), "FetchExtendedData"),
    ]
    .into_iter()
    .collect()
}
