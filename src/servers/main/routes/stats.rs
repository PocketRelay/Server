use std::sync::Arc;

use crate::{
    servers::main::{
        models::{
            errors::{ServerError, ServerResult},
            stats::*,
        },
        session::SessionLink,
    },
    services::leaderboard::{models::*, QueryMessage},
    state::GlobalState,
    utils::components::{Components as C, Stats as S},
};
use blaze_pk::{
    packet::{Request, Response},
    router::Router,
};

/// Routing function for adding all the routes in this file to the
/// provided router
///
/// `router` The router to add to
pub fn route(router: &mut Router<C, SessionLink>) {
    router.route(
        C::Stats(S::GetLeaderboardEntityCount),
        handle_leaderboard_entity_count,
    );
    router.route(C::Stats(S::GetLeaderboard), handle_normal_leaderboard);
    router.route(
        C::Stats(S::GetCenteredLeaderboard),
        handle_centered_leaderboard,
    );
    router.route(
        C::Stats(S::GetFilteredLeaderboard),
        handle_filtered_leaderboard,
    );
    router.route(C::Stats(S::GetLeaderboardGroup), handle_leaderboard_group);
}

async fn handle_normal_leaderboard(req: Request<LeaderboardRequest>) -> ServerResult<Response> {
    let query = &*req;
    let group = get_group(&query.name).await?;
    let response = match group.get_normal(query.start, query.count) {
        Some((values, _)) => LeaderboardResponse::Many(values),
        None => LeaderboardResponse::Empty,
    };
    Ok(req.response(response))
}

async fn handle_centered_leaderboard(
    req: Request<CenteredLeaderboardRequest>,
) -> ServerResult<Response> {
    let query = &*req;
    let group = get_group(&query.name).await?;
    let response = match group.get_centered(query.center, query.count) {
        Some(values) => LeaderboardResponse::Many(values),
        None => LeaderboardResponse::Empty,
    };
    Ok(req.response(response))
}
async fn handle_filtered_leaderboard(
    req: Request<FilteredLeaderboardRequest>,
) -> ServerResult<Response> {
    let query = &*req;
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
async fn handle_leaderboard_entity_count(
    req: EntityCountRequest,
) -> ServerResult<EntityCountResponse> {
    let group = get_group(&req.name).await?;
    let count = group.values.len();
    Ok(EntityCountResponse { count })
}

async fn get_group(name: &str) -> ServerResult<Arc<LeaderboardGroup>> {
    let services = GlobalState::services();
    let leaderboard = &services.leaderboard;
    let ty = LeaderboardType::from_value(name);
    leaderboard
        .send(QueryMessage(ty))
        .await
        .map_err(|_| ServerError::ServerUnavailableFinal)
}

fn get_locale_name(code: &str) -> &str {
    match &code as &str {
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
async fn handle_leaderboard_group(
    req: LeaderboardGroupRequest,
) -> Option<LeaderboardGroupResponse<'static>> {
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
    Some(group)
}
