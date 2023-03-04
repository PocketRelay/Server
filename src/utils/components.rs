//! Modules contains the component definitions for the servers used throughout
//! this application.

use blaze_pk::{PacketComponent, PacketComponents};

#[derive(Debug, Hash, PartialEq, Eq, PacketComponents)]
pub enum Components {
    #[component(target = 0x1)]
    Authentication(Authentication),
    #[component(target = 0x4)]
    GameManager(GameManager),
    #[component(target = 0x5)]
    Redirector(Redirector),
    #[component(target = 0x7)]
    Stats(Stats),
    #[component(target = 0x9)]
    Util(Util),
    #[component(target = 0xF)]
    Messaging(Messaging),
    #[component(target = 0x19)]
    AssociationLists(AssociationLists),
    #[component(target = 0x1C)]
    GameReporting(GameReporting),
    #[component(target = 0x7802)]
    UserSessions(UserSessions),
}

#[derive(Debug, Hash, PartialEq, Eq, PacketComponent)]
pub enum Authentication {
    #[command(target = 0x14)]
    UpdateAccount,
    #[command(target = 0x1C)]
    UpdateParentalEmail,
    #[command(target = 0x1D)]
    ListUserEntitlements2,
    #[command(target = 0x1E)]
    GetAccount,
    #[command(target = 0x1F)]
    GrantEntitlement,
    #[command(target = 0x20)]
    ListEntitlements,
    #[command(target = 0x21)]
    HasEntitlement,
    #[command(target = 0x22)]
    GetUseCount,
    #[command(target = 0x23)]
    DecrementUseCount,
    #[command(target = 0x24)]
    GetAuthToken,
    #[command(target = 0x25)]
    GetHandoffToken,
    #[command(target = 0x26)]
    GetPasswordRules,
    #[command(target = 0x27)]
    GrantEntitlement2,
    #[command(target = 0x28)]
    Login,
    #[command(target = 0x29)]
    AcceptTOS,
    #[command(target = 0x2A)]
    GetTOSInfo,
    #[command(target = 0x2B)]
    ModifyEntitlement2,
    #[command(target = 0x2C)]
    ConsumeCode,
    #[command(target = 0x2D)]
    PasswordForgot,
    #[command(target = 0x2E)]
    GetTOSContent,
    #[command(target = 0x2F)]
    GetPrivacyPolicyContent,
    #[command(target = 0x30)]
    ListPersonalEntitlements2,
    #[command(target = 0x32)]
    SilentLogin,
    #[command(target = 0x33)]
    CheckAgeRequirement,
    #[command(target = 0x34)]
    GetOptIn,
    #[command(target = 0x35)]
    EnableOptIn,
    #[command(target = 0x36)]
    DisableOptIn,
    #[command(target = 0x3C)]
    ExpressLogin,
    #[command(target = 0x46)]
    Logout,
    #[command(target = 0x50)]
    CreatePersona,
    #[command(target = 0x5A)]
    GetPersona,
    #[command(target = 0x64)]
    ListPersonas,
    #[command(target = 0x6E)]
    LoginPersona,
    #[command(target = 0x78)]
    LogoutPersona,
    #[command(target = 0x8C)]
    DeletePersona,
    #[command(target = 0x8D)]
    DisablePersona,
    #[command(target = 0x8F)]
    ListDeviceAccounts,
    #[command(target = 0x96)]
    XboxCreateAccount,
    #[command(target = 0x98)]
    OriginLogin,
    #[command(target = 0xA0)]
    XboxAssociateAccount,
    #[command(target = 0xAA)]
    XboxLogin,
    #[command(target = 0xB4)]
    PS3CreateAccount,
    #[command(target = 0xBE)]
    PS3AssociateAccount,
    #[command(target = 0xC8)]
    PS3Login,
    #[command(target = 0xD2)]
    ValidateSessionKey,
    #[command(target = 0xE6)]
    CreateWalUserSession,
    #[command(target = 0xF1)]
    AcceptLegalDocs,
    #[command(target = 0xF2)]
    GetLegalDocsInfo,
    #[command(target = 0xF6)]
    GetTermsOfServiceConent,
    #[command(target = 0x12C)]
    DeviceLoginGuest,
    #[command(target = 0xA)]
    CreateAccount,
}

#[derive(Debug, Hash, PartialEq, Eq, PacketComponent)]
pub enum GameManager {
    #[command(target = 0x1)]
    CreateGame,
    #[command(target = 0x2)]
    DestroyGame,
    #[command(target = 0x3)]
    AdvanceGameState,
    #[command(target = 0x4)]
    SetGameSettings,
    #[command(target = 0x5)]
    SetPlayerCapacity,
    #[command(target = 0x6)]
    SetPresenceMode,
    #[command(target = 0x7)]
    SetGameAttributes,
    #[command(target = 0x8)]
    SetPlayerAttributes,
    #[command(target = 0x9)]
    JoinGame,
    #[command(target = 0xB)]
    RemovePlayer,
    #[command(target = 0xD)]
    StartMatchmaking,
    #[command(target = 0xE)]
    CancelMatchmaking,
    #[command(target = 0xF)]
    FinalizeGameCreation,
    #[command(target = 0x11)]
    ListGames,
    #[command(target = 0x12)]
    SetPlayerCustomData,
    #[command(target = 0x13)]
    ReplayGame,
    #[command(target = 0x14)]
    ReturnDedicatedServerToPool,
    #[command(target = 0x15)]
    JoinGameByGroup,
    #[command(target = 0x16)]
    LeaveGameByGroup,
    #[command(target = 0x17)]
    MigrateGame,
    #[command(target = 0x18)]
    UpdateGameHostMigrationStatus,
    #[command(target = 0x19)]
    ResetDedicatedServe,
    #[command(target = 0x1A)]
    UpdateGameSession,
    #[command(target = 0x1B)]
    BanPlayer,
    #[command(target = 0x1D)]
    UpdateMeshConnection,
    #[command(target = 0x1F)]
    RemovePlayerFromBannedList,
    #[command(target = 0x20)]
    ClearBannedList,
    #[command(target = 0x21)]
    GetBannedList,
    #[command(target = 0x26)]
    AddQueuedPlayerToGame,
    #[command(target = 0x27)]
    UpdateGameName,
    #[command(target = 0x28)]
    EjectHost,
    #[command(target = 0x64)]
    GetGameListSnapshot,
    #[command(target = 0x65)]
    GetGameListSubscription,
    #[command(target = 0x66)]
    DestroyGameList,
    #[command(target = 0x67)]
    GetFullGameData,
    #[command(target = 0x68)]
    GetMatchmakingConfig,
    #[command(target = 0x69)]
    GetGameDataFromID,
    #[command(target = 0x6A)]
    AddAdminPlayer,
    #[command(target = 0x6B)]
    RemoveAdminPlayer,
    #[command(target = 0x6C)]
    SetPlayerTeam,
    #[command(target = 0x6D)]
    ChangeGameTeamID,
    #[command(target = 0x6E)]
    MigrateAdminPlayer,
    #[command(target = 0x6F)]
    GetUserSetGameListSubscription,
    #[command(target = 0x70)]
    SwapPlayersTeam,
    #[command(target = 0x96)]
    RegisterDynamicDedicatedServerCreator,
    #[command(target = 0x97)]
    UnregisterDynamicDedicatedServerCreator,

    #[command(target = 0xA, notify)]
    MatchmakingFailed,
    #[command(target = 0xC, notify)]
    MatchmakingAsyncStatus,
    #[command(target = 0xF, notify)]
    GameCreated,
    #[command(target = 0x10, notify)]
    GameRemoved,
    #[command(target = 0x14, notify)]
    GameSetup,
    #[command(target = 0x15, notify)]
    PlayerJoining,
    #[command(target = 0x16, notify)]
    JoiningPlayerInitiateConnections,
    #[command(target = 0x17, notify)]
    PlayerJoiningQueue,
    #[command(target = 0x18, notify)]
    PlayerPromotedFromQueue,
    #[command(target = 0x19, notify)]
    PlayerClaimingReservation,
    #[command(target = 0x1E, notify)]
    PlayerJoinCompleted,
    #[command(target = 0x28, notify)]
    PlayerRemoved,
    #[command(target = 0x3C, notify)]
    HostMigrationFinished,
    #[command(target = 0x46, notify)]
    HostMigrationStart,
    #[command(target = 0x47, notify)]
    PlatformHostInitialized,
    #[command(target = 0x50, notify)]
    GameAttribChange,
    #[command(target = 0x5A, notify)]
    PlayerAttribChange,
    #[command(target = 0x5F, notify)]
    PlayerCustomDataChange,
    #[command(target = 0x64, notify)]
    GameStateChange,
    #[command(target = 0x6E, notify)]
    GameSettingsChange,
    #[command(target = 0x6F, notify)]
    GameCapacityChange,
    #[command(target = 0x70, notify)]
    GameReset,
    #[command(target = 0x71, notify)]
    GameReportingIDChange,
    #[command(target = 0x73, notify)]
    GameSessionUpdated,
    #[command(target = 0x74, notify)]
    GamePlayerStateChange,
    #[command(target = 0x75, notify)]
    GamePlayerTeamChange,
    #[command(target = 0x76, notify)]
    GameTeamIDChange,
    #[command(target = 0x77, notify)]
    ProcesssQueue,
    #[command(target = 0x78, notify)]
    PrecenseModeChanged,
    #[command(target = 0x79, notify)]
    GamePlayerQueuePositionChange,
    #[command(target = 0xC9, notify)]
    GameListUpdate,
    #[command(target = 0xCA, notify)]
    AdminListChange,
    #[command(target = 0xDC, notify)]
    CreateDynamicDedicatedServerGame,
    #[command(target = 0xE6, notify)]
    GameNameChange,
}

#[derive(Debug, Hash, PartialEq, Eq, PacketComponent)]
pub enum Redirector {
    #[command(target = 0x1)]
    GetServerInstance,
}

#[derive(Debug, Hash, PartialEq, Eq, PacketComponent)]
pub enum Stats {
    #[command(target = 0x1)]
    GetStatDecs,
    #[command(target = 0x2)]
    GetStats,
    #[command(target = 0x3)]
    GetStatGroupList,
    #[command(target = 0x4)]
    GetStatGroup,
    #[command(target = 0x5)]
    GetStatsByGroup,
    #[command(target = 0x6)]
    GetDateRange,
    #[command(target = 0x7)]
    GetEntityCount,
    #[command(target = 0xA)]
    GetLeaderboardGroup,
    #[command(target = 0xB)]
    GetLeaderboardFolderGroup,
    #[command(target = 0xC)]
    GetLeaderboard,
    #[command(target = 0xD)]
    GetCenteredLeaderboard,
    #[command(target = 0xE)]
    GetFilteredLeaderboard,
    #[command(target = 0xF)]
    GetKeyScopesMap,
    #[command(target = 0x10)]
    GetStatsByGroupASync,
    #[command(target = 0x11)]
    GetLeaderboardTreeAsync,
    #[command(target = 0x12)]
    GetLeaderboardEntityCount,
    #[command(target = 0x13)]
    GetStatCategoryList,
    #[command(target = 0x14)]
    GetPeriodIDs,
    #[command(target = 0x15)]
    GetLeaderboardRaw,
    #[command(target = 0x16)]
    GetCenteredLeaderboardRaw,
    #[command(target = 0x17)]
    GetFilteredLeaderboardRaw,
    #[command(target = 0x18)]
    ChangeKeyScopeValue,
}

#[derive(Debug, Hash, PartialEq, Eq, PacketComponent)]
pub enum Util {
    #[command(target = 0x1)]
    FetchClientConfig,
    #[command(target = 0x2)]
    Ping,
    #[command(target = 0x3)]
    SetClientData,
    #[command(target = 0x4)]
    LocalizeStrings,
    #[command(target = 0x5)]
    GetTelemetryServer,
    #[command(target = 0x6)]
    GetTickerServer,
    #[command(target = 0x7)]
    PreAuth,
    #[command(target = 0x8)]
    PostAuth,
    #[command(target = 0xA)]
    UserSettingsLoad,
    #[command(target = 0xB)]
    UserSettingsSave,
    #[command(target = 0xC)]
    UserSettingsLoadAll,
    #[command(target = 0xE)]
    DeleteUserSettings,
    #[command(target = 0x14)]
    FilterForProfanity,
    #[command(target = 0x15)]
    FetchQOSConfig,
    #[command(target = 0x16)]
    SetClientMetrics,
    #[command(target = 0x17)]
    SetConnectionState,
    #[command(target = 0x18)]
    GetPSSConfig,
    #[command(target = 0x19)]
    GetUserOptions,
    #[command(target = 0x1A)]
    SetUserOptions,
    #[command(target = 0x1B)]
    SuspendUserPing,
}

#[derive(Debug, Hash, PartialEq, Eq, PacketComponent)]
pub enum Messaging {
    #[command(target = 0x2)]
    FetchMessages,
    #[command(target = 0x3)]
    PurgeMessages,
    #[command(target = 0x4)]
    TouchMessages,
    #[command(target = 0x5)]
    GetMessages,
    #[command(target = 0x1, notify)]
    SendMessage,
}

#[derive(Debug, Hash, PartialEq, Eq, PacketComponent)]
pub enum AssociationLists {
    #[command(target = 0x1)]
    AddUsersToList,
    #[command(target = 0x2)]
    RemoveUsersFromList,
    #[command(target = 0x3)]
    ClearList,
    #[command(target = 0x4)]
    SetUsersToList,
    #[command(target = 0x5)]
    GetListForUser,
    #[command(target = 0x6)]
    GetLists,
    #[command(target = 0x7)]
    SubscribeToLists,
    #[command(target = 0x8)]
    UnsubscribeToLists,
    #[command(target = 0x9)]
    GetConfigListsInfo,
}

#[derive(Debug, Hash, PartialEq, Eq, PacketComponent)]
pub enum GameReporting {
    #[command(target = 0x1)]
    SubmitGameReport,
    #[command(target = 0x2)]
    SubmitOfflineGameReport,
    #[command(target = 0x3)]
    SubmitGameEvents,
    #[command(target = 0x4)]
    GetGameReportQuery,
    #[command(target = 0x5)]
    GetGameReportQueriesList,
    #[command(target = 0x6)]
    GetGameReports,
    #[command(target = 0x7)]
    GetGameReportView,
    #[command(target = 0x8)]
    GetGameReportViewInfo,
    #[command(target = 0x9)]
    GetGameReportViewInfoList,
    #[command(target = 0xA)]
    GetGameReportTypes,
    #[command(target = 0xB)]
    UpdateMetrics,
    #[command(target = 0xC)]
    GetGameReportColumnInfo,
    #[command(target = 0xD)]
    GetGameReortColummnValues,
    #[command(target = 0x64)]
    SubmitTrustedMidGameReport,
    #[command(target = 0x65)]
    SubmitTrustedEndGameReport,
    #[command(target = 0x72, notify)]
    GameReportSubmitted,
}

#[derive(Debug, Hash, PartialEq, Eq, PacketComponent)]
pub enum UserSessions {
    #[command(target = 0x8)]
    UpdateHardwareFlags,
    #[command(target = 0xC)]
    LookupUser,
    #[command(target = 0xD)]
    LookupUsers,
    #[command(target = 0xE)]
    LookupUsersByPrefix,
    #[command(target = 0x14)]
    UpdateNetworkInfo,
    #[command(target = 0x17)]
    LookupUserGeoIPData,
    #[command(target = 0x18)]
    OverrideUserGeoIPData,
    #[command(target = 0x19)]
    UpdateUserSessionClientData,
    #[command(target = 0x1A)]
    SetUserInfoAttribute,
    #[command(target = 0x1B)]
    ResetUserGeoIPData,
    #[command(target = 0x20)]
    LookupUserSessionID,
    #[command(target = 0x21)]
    FetchLastLocaleUsedAndAuthError,
    #[command(target = 0x22)]
    FetchUserFirstLastAuthTime,
    #[command(target = 0x23)]
    ResumeSession,
    #[command(target = 0x1, notify)]
    SetSession,
    #[command(target = 0x2, notify)]
    SessionDetails,
    #[command(target = 0x5, notify)]
    UpdateExtendedDataAttribute,
    #[command(target = 0x3, notify)]
    FetchExtendedData,
}
