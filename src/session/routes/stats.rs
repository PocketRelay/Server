use crate::{
    services::leaderboard::{models::*, QueryMessage},
    session::{
        models::{errors::ServerResult, stats::*},
        packet::PacketResponse,
        router::{Blaze, BlazeWithHeader},
    },
    state::App,
};
use std::sync::Arc;

pub async fn handle_normal_leaderboard(
    req: BlazeWithHeader<LeaderboardRequest>,
) -> ServerResult<PacketResponse> {
    let query = &req.req;
    let group = get_group(&query.name).await?;
    let response = match group.get_normal(query.start, query.count) {
        Some((values, _)) => LeaderboardResponse::Many(values),
        None => LeaderboardResponse::Empty,
    };
    Ok(req.response(response))
}

pub async fn handle_centered_leaderboard(
    req: BlazeWithHeader<CenteredLeaderboardRequest>,
) -> ServerResult<PacketResponse> {
    let query = &req.req;
    let group = get_group(&query.name).await?;
    let response = match group.get_centered(query.center, query.count) {
        Some(values) => LeaderboardResponse::Many(values),
        None => LeaderboardResponse::Empty,
    };
    Ok(req.response(response))
}

pub async fn handle_filtered_leaderboard(
    req: BlazeWithHeader<FilteredLeaderboardRequest>,
) -> ServerResult<PacketResponse> {
    let query = &req.req;
    let group = get_group(&query.name).await?;
    let response = match group.get_entry(query.id) {
        Some(value) => LeaderboardResponse::One(value),
        None => LeaderboardResponse::Empty,
    };
    Ok(req.response(response))
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
    Blaze(req): Blaze<EntityCountRequest>,
) -> ServerResult<Blaze<EntityCountResponse>> {
    let group = get_group(&req.name).await?;
    let count = group.values.len();
    Ok(Blaze(EntityCountResponse { count }))
}

async fn get_group(name: &str) -> ServerResult<Arc<LeaderboardGroup>> {
    let services = App::services();
    let leaderboard = &services.leaderboard;
    let ty = LeaderboardType::from_value(name);
    let result = leaderboard.send(QueryMessage(ty)).await?;
    Ok(result)
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
