use std::sync::Arc;
use blaze_pk::OpaquePacket;
use crate::blaze::components::Stats;
use crate::blaze::routes::HandleResult;
use crate::blaze::Session;

pub async fn route(_session: Arc<Session>, component: Stats, _packet: OpaquePacket) -> HandleResult {
    match component {
        Stats::GetStatDecs => {}
        Stats::GetStats => {}
        Stats::GetStatGroupList => {}
        Stats::GetStatGroup => {}
        Stats::GetStatsByGroup => {}
        Stats::GetDateRange => {}
        Stats::GetEntityCount => {}
        Stats::GetLeaderboardGroup => {}
        Stats::GetLeaderboardFolderGroup => {}
        Stats::GetLeaderboard => {}
        Stats::GetCenteredLeaderboard => {}
        Stats::GetFilteredLeaderboard => {}
        Stats::GetKeyScopesMap => {}
        Stats::GetStatsByGroupASync => {}
        Stats::GetLeaderboardTreeAsync => {}
        Stats::GetLeaderboardEntityCount => {}
        Stats::GetStatCategoryList => {}
        Stats::GetPeriodIDs => {}
        Stats::GetLeaderboardRaw => {}
        Stats::GetCenteredLeaderboardRaw => {}
        Stats::GetFilteredLeaderboardRaw => {}
        Stats::ChangeKeyScopeValue => {}
        Stats::Unknown(_) => {}
    }
    Ok(())
}