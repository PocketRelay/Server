use crate::{
    database::entities::LeaderboardData,
    session::{
        models::stats::*,
        router::{Blaze, Extension},
    },
};
use sea_orm::DatabaseConnection;

pub async fn handle_normal_leaderboard(
    Extension(db): Extension<DatabaseConnection>,
    Blaze(query): Blaze<LeaderboardRequest>,
) -> Blaze<LeaderboardResponse> {
    let values = LeaderboardData::get_offset(&db, query.name, query.start, query.count)
        .await
        .unwrap_or_default();
    Blaze(LeaderboardResponse { values })
}

pub async fn handle_centered_leaderboard(
    Extension(db): Extension<DatabaseConnection>,
    Blaze(query): Blaze<CenteredLeaderboardRequest>,
) -> Blaze<LeaderboardResponse> {
    let values = LeaderboardData::get_centered(&db, query.name, query.center, query.count)
        .await
        .unwrap_or_default()
        .unwrap_or_default();

    Blaze(LeaderboardResponse { values })
}

pub async fn handle_filtered_leaderboard(
    Extension(db): Extension<DatabaseConnection>,
    Blaze(query): Blaze<FilteredLeaderboardRequest>,
) -> Blaze<LeaderboardResponse> {
    let values = LeaderboardData::get_filtered(&db, query.name, query.ids)
        .await
        .unwrap_or_default();

    Blaze(LeaderboardResponse { values })
}

/// Handles returning the number of leaderboard objects present.
///
/// ```
/// Route: Stats(GetLeaderboardEntityCount)
/// ID: 0
/// Content: {
///     "KSUM": Map {
///         "accountcountry": 0,
///         "ME3Map": 0
///     },
///     "LBID": 0,
///     "NAME": "N7RatingGlobal",
///     "POFF": 0
/// }
/// ```
pub async fn handle_leaderboard_entity_count(
    Extension(db): Extension<DatabaseConnection>,
    Blaze(req): Blaze<EntityCountRequest>,
) -> Blaze<EntityCountResponse> {
    let total = LeaderboardData::count(&db, req.name)
        .await
        .unwrap_or_default();

    Blaze(EntityCountResponse {
        count: total as usize,
    })
}

fn get_locale_name(code: &str) -> &str {
    match code as &str {
        "global" => "Global",
        "de" => "Germany",
        "en" => "English",
        "es" => "Spain",
        "fr" => "France",
        "it" => "Italy",
        "ja" => "Japan",
        "pl" => "Poland",
        "ru" => "Russia",
        "nz" => "New Zealand",
        value => value,
    }
}

///
///
/// ```
/// Route: Stats(GetLeaderboardGroup)
/// ID: 19
/// Content: {
///     "LBID": 1,
///     "NAME": "N7RatingGlobal"
/// }
/// ```
pub async fn handle_leaderboard_group(
    Blaze(req): Blaze<LeaderboardGroupRequest>,
) -> Option<Blaze<LeaderboardGroupResponse<'static>>> {
    let name = req.name;
    let is_n7 = name.starts_with("N7Rating");
    if !is_n7 && !name.starts_with("ChallengePoints") {
        return None;
    }
    let split = if is_n7 { 8 } else { 15 };
    let local_code = name.split_at(split).1.to_lowercase();
    let locale = get_locale_name(&local_code);
    let group = if is_n7 {
        let desc = format!("N7 Rating - {locale}");
        LeaderboardGroupResponse {
            name,
            desc,
            sname: "n7rating",
            sdsc: "N7 Rating",
            gname: "ME3LeaderboardGroup",
        }
    } else {
        let desc = format!("Challenge Points - {locale}");
        LeaderboardGroupResponse {
            name,
            desc,
            sname: "ChallengePoints",
            sdsc: "Challenge Points",
            gname: "ME3ChallengePoints",
        }
    };
    Some(Blaze(group))
}
