use std::sync::Arc;
use blaze_pk::OpaquePacket;
use crate::blaze::components::GameManager;
use crate::blaze::routes::HandleResult;
use crate::blaze::Session;

pub async fn route(_session: Arc<Session>, component: GameManager, _packet: OpaquePacket) -> HandleResult{
    match component {
        GameManager::CreateGame => {}
        GameManager::DestroyGame => {}
        GameManager::AdvanceGameState => {}
        GameManager::SetGameSettings => {}
        GameManager::SetPlayerCapacity => {}
        GameManager::SetPresenceMode => {}
        GameManager::SetGameAttributes => {}
        GameManager::SetPlayerAttributes => {}
        GameManager::JoinGame => {}
        GameManager::RemovePlayer => {}
        GameManager::StartMatchaking => {}
        GameManager::CancelMatchmaking => {}
        GameManager::FinalizeGameCreation => {}
        GameManager::ListGames => {}
        GameManager::SetPlayerCustomData => {}
        GameManager::ReplayGame => {}
        GameManager::ReturnDedicatedServerToPool => {}
        GameManager::JoinGameByGroup => {}
        GameManager::LeaveGameByGroup => {}
        GameManager::MigrateGame => {}
        GameManager::UpdateGameHostMigrationStatus => {}
        GameManager::ResetDedicatedServer => {}
        GameManager::UpdateGameSession => {}
        GameManager::BanPlayer => {}
        GameManager::UpdateMeshConnection => {}
        GameManager::RemovePlayerFromBannedList => {}
        GameManager::ClearBannedList => {}
        GameManager::GetBannedList => {}
        GameManager::AddQueuedPlayerToGame => {}
        GameManager::UpdateGameName => {}
        GameManager::EjectHost => {}
        GameManager::NotifyGameUpdated => {}
        GameManager::GetGameListSnapshot => {}
        GameManager::GetGameListSubscription => {}
        GameManager::DestroyGameList => {}
        GameManager::GetFullGameData => {}
        GameManager::GetMatchmakingConfig => {}
        GameManager::GetGameDataFromID => {}
        GameManager::AddAdminPlayer => {}
        GameManager::RemoveAdminPlayer => {}
        GameManager::SetPlayerTeam => {}
        GameManager::ChangeGameTeamID => {}
        GameManager::MigrateAdminPlayer => {}
        GameManager::GetUserSetGameListSubscription => {}
        GameManager::SwapPlayersTeam => {}
        GameManager::RegisterDynamicDedicatedServerCreator => {}
        GameManager::UnregisterDynamicDedicatedServerCreator => {}
        GameManager::MatchmakingFailed => {}
        GameManager::MatchmakingAsyncStatus => {}
        GameManager::GameCreated => {}
        GameManager::GameRemoved => {}
        GameManager::GameSetup => {}
        GameManager::PlayerJoining => {}
        GameManager::JoiningPlayerInitiateConnections => {}
        GameManager::PlayerJoiningQueue => {}
        GameManager::PlayerPromotedFromQueue => {}
        GameManager::PlayerClaimingReservation => {}
        GameManager::PlayerJoinCompleted => {}
        GameManager::PlayerRemoved => {}
        GameManager::HostMigrationFinished => {}
        GameManager::HostMigrationStart => {}
        GameManager::PlatformHostInitialized => {}
        GameManager::GameAttribChange => {}
        GameManager::PlayerAttribChange => {}
        GameManager::PlayerCustomDataChange => {}
        GameManager::GameStateChange => {}
        GameManager::GameSettingsChange => {}
        GameManager::GameCapacityChange => {}
        GameManager::GameReset => {}
        GameManager::GameReportingIDChange => {}
        GameManager::GameSessionUpdated => {}
        GameManager::GamePlayerStateChange => {}
        GameManager::GamePlayerTeamChange => {}
        GameManager::GameTeamIDChange => {}
        GameManager::ProcesssQueue => {}
        GameManager::PrecenseModeChanged => {}
        GameManager::GamePlayerQueuePositionChange => {}
        GameManager::GameListUpdate => {}
        GameManager::AdminListChange => {}
        GameManager::CreateDynamicDedicatedServerGame => {}
        GameManager::GameNameChange => {}
        GameManager::Unknown(_) => {}
    }
    Ok(())
}