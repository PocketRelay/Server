use crate::{
    services::leaderboard::Leaderboard,
    session::{
        models::stats::*,
        packet::Packet,
        router::{Blaze, BlazeWithHeader, Extension},
    },
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub async fn handle_normal_leaderboard(
    Extension(leaderboard): Extension<Arc<Leaderboard>>,
    Extension(db): Extension<DatabaseConnection>,
    req: BlazeWithHeader<LeaderboardRequest>,
) -> Packet {
    let query = &req.req;
    let group = leaderboard.query(query.name, &db).await;
    let slice = group
        .get_normal(query.start, query.count)
        .unwrap_or_default();
    req.response(LeaderboardResponse::Borrowed(slice))
}

pub async fn handle_centered_leaderboard(
    Extension(leaderboard): Extension<Arc<Leaderboard>>,
    Extension(db): Extension<DatabaseConnection>,
    req: BlazeWithHeader<CenteredLeaderboardRequest>,
) -> Packet {
    let query = &req.req;
    let group = leaderboard.query(query.name, &db).await;
    let slice = group
        .get_centered(query.center, query.count)
        .unwrap_or_default();
    req.response(LeaderboardResponse::Borrowed(slice))
}

pub async fn handle_filtered_leaderboard(
    Extension(leaderboard): Extension<Arc<Leaderboard>>,
    Extension(db): Extension<DatabaseConnection>,
    req: BlazeWithHeader<FilteredLeaderboardRequest>,
) -> Packet {
    let query = &req.req;
    let group = leaderboard.query(query.name, &db).await;
    let response = group.get_filtered(&query.ids);
    req.response(LeaderboardResponse::Owned(response))
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
    Extension(leaderboard): Extension<Arc<Leaderboard>>,
    Extension(db): Extension<DatabaseConnection>,
    Blaze(req): Blaze<EntityCountRequest>,
) -> Blaze<EntityCountResponse> {
    let group = leaderboard.query(req.name, &db).await;
    let count = group.values.len();
    Blaze(EntityCountResponse { count })
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
