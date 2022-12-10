//! Modules contains the component definitions for the servers used throughout
//! this application.

use blaze_pk;
use blaze_pk::define_components;

define_components! {
    Authentication (0x1) {
        UpdateAccount (0x14)
        UpdateParentalEmail (0x1C)
        ListUserEntitlements2 (0x1D)
        GetAccount (0x1E)
        GrantEntitlement (0x1F)
        ListEntitlements (0x20)
        HasEntitlement (0x21)
        GetUseCount (0x22)
        DecrementUseCount (0x23)
        GetAuthToken(0x24)
        GetHandoffToken (0x25)
        GetPasswordRules (0x26)
        GrantEntitlement2 (0x27)
        Login (0x28)
        AcceptTOS (0x29)
        GetTOSInfo (0x2A)
        ModifyEntitlement2 (0x2B)
        ConsumeCode (0x2C)
        PasswordForgot (0x2D)
        GetTOSContent (0x2E)
        GetPrivacyPolicyContent (0x2F)
        ListPersonalEntitlements2 (0x30)
        SilentLogin (0x32)
        CheckAgeRequirement (0x33)
        GetOptIn (0x34)
        EnableOptIn (0x35)
        DisableOptIn (0x36)
        ExpressLogin (0x3C)
        Logout (0x46)
        CreatePersona (0x50)
        GetPersona (0x5A)
        ListPersonas (0x64)
        LoginPersona (0x6E)
        LogoutPersona (0x78)
        DeletePersona (0x8C)
        DisablePersona (0x8D)
        ListDeviceAccounts (0x8F)
        XboxCreateAccount (0x96)
        OriginLogin (0x98)
        XboxAssociateAccount (0xA0)
        XboxLogin (0xAA)
        PS3CreateAccount (0xB4)
        PS3AssociateAccount (0xBE)
        PS3Login (0xC8)
        ValidateSessionKey (0xD2)
        CreateWalUserSession (0xE6)
        AcceptLegalDocs (0xF1)
        GetLegalDocsInfo (0xF2)
        GetTermsOfServiceConent (0xF6)
        DeviceLoginGuest (0x12C)
        CreateAccount (0xA)
    }

    GameManager (0x4) {
        CreateGame (0x1)
        DestroyGame (0x2)
        AdvanceGameState (0x3)
        SetGameSettings (0x4)
        SetPlayerCapacity (0x5)
        SetPresenceMode (0x6)
        SetGameAttributes (0x7)
        SetPlayerAttributes (0x8)
        JoinGame (0x9)
        RemovePlayer (0xB)
        StartMatchmaking (0xD)
        CancelMatchmaking (0xE)
        FinalizeGameCreation (0xF)
        ListGames (0x11)
        SetPlayerCustomData (0x12)
        ReplayGame (0x13)
        ReturnDedicatedServerToPool (0x14)
        JoinGameByGroup (0x15)
        LeaveGameByGroup (0x16)
        MigrateGame (0x17)
        UpdateGameHostMigrationStatus (0x18)
        ResetDedicatedServer (0x19)
        UpdateGameSession (0x1A)
        BanPlayer (0x1B)
        UpdateMeshConnection (0x1D)
        RemovePlayerFromBannedList (0x1F)
        ClearBannedList(0x20)
        GetBannedList(0x21)
        AddQueuedPlayerToGame(0x26)
        UpdateGameName(0x27)
        EjectHost(0x28)
        GetGameListSnapshot(0x64)
        GetGameListSubscription(0x65)
        DestroyGameList(0x66)
        GetFullGameData(0x67)
        GetMatchmakingConfig(0x68)
        GetGameDataFromID(0x69)
        AddAdminPlayer(0x6A)
        RemoveAdminPlayer(0x6B)
        SetPlayerTeam(0x6C)
        ChangeGameTeamID (0x6D)
        MigrateAdminPlayer(0x6E)
        GetUserSetGameListSubscription(0x6F)
        SwapPlayersTeam(0x70)
        RegisterDynamicDedicatedServerCreator(0x96)
        UnregisterDynamicDedicatedServerCreator(0x97);

        notify {
            MatchmakingFailed (0xA)
            MatchmakingAsyncStatus (0xC)
            GameCreated (0xF)
            GameRemoved (0x10)
            GameSetup (0x14)
            PlayerJoining (0x15)
            JoiningPlayerInitiateConnections (0x16)
            PlayerJoiningQueue (0x17)
            PlayerPromotedFromQueue (0x18)
            PlayerClaimingReservation (0x19)
            PlayerJoinCompleted (0x1E)
            PlayerRemoved (0x28)
            HostMigrationFinished (0x3C)
            HostMigrationStart (0x46)
            PlatformHostInitialized (0x47)
            GameAttribChange (0x50)
            PlayerAttribChange(0x5A)
            PlayerCustomDataChange (0x5F)
            GameStateChange (0x64)
            GameSettingsChange (0x6E)
            GameCapacityChange(0x6F)
            GameReset (0x70)
            GameReportingIDChange (0x71)
            GameSessionUpdated (0x73)
            GamePlayerStateChange (0x74)
            GamePlayerTeamChange (0x75)
            GameTeamIDChange (0x76)
            ProcesssQueue (0x77)
            PrecenseModeChanged (0x78)
            GamePlayerQueuePositionChange (0x79)
            GameListUpdate (0xC9)
            AdminListChange (0xCA)
            CreateDynamicDedicatedServerGame (0xDC)
            GameNameChange (0xE6)
        }
    }

    Redirector(0x5) {
        GetServerInstance (0x1)
    }

    Stats (0x7) {
        GetStatDecs(0x1)
        GetStats(0x2)
        GetStatGroupList(0x3)
        GetStatGroup(0x4)
        GetStatsByGroup(0x5)
        GetDateRange(0x6)
        GetEntityCount(0x7)
        GetLeaderboardGroup(0xA)
        GetLeaderboardFolderGroup(0xB)
        GetLeaderboard(0xc)
        GetCenteredLeaderboard(0xD)
        GetFilteredLeaderboard(0xE)
        GetKeyScopesMap(0xF)
        GetStatsByGroupASync(0x10)
        GetLeaderboardTreeAsync(0x11)
        GetLeaderboardEntityCount(0x12)
        GetStatCategoryList(0x13)
        GetPeriodIDs(0x14)
        GetLeaderboardRaw(0x15)
        GetCenteredLeaderboardRaw(0x16)
        GetFilteredLeaderboardRaw(0x17)
        ChangeKeyScopeValue(0x18)
    }

    Util (0x9) {
        FetchClientConfig (0x1)
        Ping (0x2)
        SetClientData (0x3)
        LocalizeStrings (0x4)
        GetTelemetryServer (0x5)
        GetTickerServer (0x6)
        PreAuth (0x7)
        PostAuth (0x8)
        UserSettingsLoad (0xA)
        UserSettingsSave (0xB)
        UserSettingsLoadAll (0xC)
        DeleteUserSettings (0xE)
        FilterForProfanity (0x14)
        FetchQOSConfig (0x15)
        SetClientMetrics (0x16)
        SetConnectionState (0x17)
        GetPSSConfig (0x18)
        GetUserOptions (0x19)
        SetUserOptions (0x1A)
        SuspendUserPing (0x1B)
    }

    Messaging (0xF) {
        FetchMessages (0x2)
        PurgeMessages (0x3)
        TouchMessages (0x4)
        GetMessages (0x5);

        notify {
            SendMessage (0x1)
        }
    }

    AssociationLists (0x19) {
        AddUsersToList (0x1)
        RemoveUsersFromList (0x2)
        ClearList (0x3)
        SetUsersToList (0x4)
        GetListForUser (0x5)
        GetLists (0x6)
        SubscribeToLists (0x7)
        UnsubscribeToLists (0x8)
        GetConfigListsInfo (0x9)
    }

    GameReporting (0x1C) {
        SubmitGameReport (0x1)
        SubmitOfflineGameReport (0x2)
        SubmitGameEvents (0x3)
        GetGameReportQuery (0x4)
        GetGameReportQueriesList (0x5)
        GetGameReports (0x6)
        GetGameReportView (0x7)
        GetGameReportViewInfo (0x8)
        GetGameReportViewInfoList (0x9)
        GetGameReportTypes (0xA)
        UpdateMetrics (0xB)
        GetGameReportColumnInfo(0xC)
        GetGameReortColummnValues(0xD)
        SubmitTrustedMidGameReport (0x64)
        SubmitTrustedEndGameReport (0x65);

        notify {
            GameReportSubmitted(0x72)
        }
    }

    UserSessions (0x7802) {
        UpdateHardwareFlags (0x8)
        LookupUser (0xC)
        LookupUsers (0xD)
        LookupUsersByPrefix (0xE)
        UpdateNetworkInfo (0x14)
        LookupUserGeoIPData (0x17)
        OverrideUserGeoIPData(0x18)
        UpdateUserSessionClientData (0x19)
        SetUserInfoAttribute (0x1A)
        ResetUserGeoIPData (0x1B)
        LookupUserSessionID (0x20)
        FetchLastLocaleUsedAndAuthError (0x21)
        FetchUserFirstLastAuthTime (0x22)
        ResumeSession (0x23);

        notify {
            SetSession (0x1)
            SessionDetails (0x2)
            UpdateExtendedDataAttribute (0x5)
            FetchExtendedData (0x3)
        }
    }
}
