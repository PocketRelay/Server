use std::collections::HashMap;

/// Creates a key for looking up a command name
const fn debug_key(component: u16, command: u16) -> u32 {
    ((component as u32) << 16) + command as u32
}

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
    pub const PLAYER_JOIN_COMPLETE: u16 = 0x1E;
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
    pub const GAME_TEAM_ID_GHANGE: u16 = 0x76;
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

    pub const FETCH_CLIENT_CONFIG: u16 = 0x1;
    pub const _: u16 = 0x2;
    pub const _: u16 = 0x3;
    pub const _: u16 = 0x4;
    pub const _: u16 = 0x5;
    pub const _: u16 = 0x6;
    pub const _: u16 = 0x7;
    pub const _: u16 = 0x8;
    // 0x9 --
    pub const _: u16 = 0xA;
    pub const _: u16 = 0xB;
    pub const _: u16 = 0xC;
    // 0xD --
    pub const _: u16 = 0xE;
    //
    pub const _: u16 = 0x14;
    pub const _: u16 = 0x15;
    pub const _: u16 = 0x16;
    pub const _: u16 = 0x17;
    pub const _: u16 = 0x18;
    pub const _: u16 = 0x19;
    pub const _: u16 = 0x1A;
    pub const _: u16 = 0x1B;
}

pub mod messaging {
    pub const COMPONENT: u16 = 0xF;
}

pub mod association_lists {
    pub const COMPONENT: u16 = 0x19;
}

pub mod game_reporting {
    pub const COMPONENT: u16 = 0x1C;
}

pub mod user_sessions {
    pub const COMPONENT: u16 = 0x7802;
}

#[rustfmt::skip]
fn components() -> HashMap<u32, &'static str> {
    use authentication as a;

    [
        // Authentication
        (debug_key(a::COMPONENT, a::UPDATE_ACCOUNT), "UpdateAccount"),
        (debug_key(a::COMPONENT, a::UPDATE_PARENTAL_EMAIL), "UpdateParentalEmail"),
        (debug_key(a::COMPONENT, a::LIST_USER_ENTITLEMENTS_2), "ListUserEntitlements2"),
      
      
    ]
    .into_iter()
    .collect()
}
