use crate::{
    servers::main::{models::stats::*, session::Session},
    services::leaderboard::models::*,
    state::GlobalState,
    utils::components::{Components as C, Stats as S},
};
use blaze_pk::{
    codec::Decodable,
    packet::{Request, Response},
    router::Router,
};

/// Routing function for adding all the routes in this file to the
/// provided router
///
/// `router` The router to add to
pub fn route(router: &mut Router<C, Session>) {
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

async fn handle_normal_leaderboard(req: Request<LeaderboardRequest>) -> Response {
    let query = &*req;
    handle_leaderboard_query(
        &query.name,
        LQuery::Normal {
            start: query.start,
            count: query.count,
        },
        &req,
    )
    .await
}

async fn handle_centered_leaderboard(req: Request<CenteredLeaderboardRequest>) -> Response {
    let query = &*req;
    handle_leaderboard_query(
        &query.name,
        LQuery::Centered {
            player_id: query.center,
            count: query.count,
        },
        &req,
    )
    .await
}
async fn handle_filtered_leaderboard(req: Request<FilteredLeaderboardRequest>) -> Response {
    let query = &*req;
    handle_leaderboard_query(
        &query.name,
        LQuery::Filtered {
            player_id: query.id,
        },
        &req,
    )
    .await
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
async fn handle_leaderboard_entity_count(req: EntityCountRequest) -> EntityCountResponse {
    let services = GlobalState::services();
    let leaderboard = &services.leaderboard;
    let ty = LeaderboardType::from_value(&req.name);

    let lock = leaderboard.get(ty).await;
    let group = lock.read().await;
    let count = group.values.len();

    EntityCountResponse { count }
}

/// Handler function for handling leaderboard querys and returning the resulting
/// leaderboard
///
/// `name`  The name of the leaderboard
/// `query` The query to resolve
async fn handle_leaderboard_query<R: Decodable>(
    name: &str,
    query: LQuery,
    req: &Request<R>,
) -> Response {
    let services = GlobalState::services();
    let leaderboard = &services.leaderboard;
    let ty = LeaderboardType::from_value(name);
    let lock = leaderboard.get(ty).await;
    let group = lock.read().await;
    let response = match group.resolve(query) {
        LResult::Many(values, _) => LeaderboardResponse::Many(values),
        LResult::One(value) => LeaderboardResponse::One(value),
        LResult::Empty => LeaderboardResponse::Empty,
    };

    req.response(response)
}

fn get_locale_name(code: &str) -> &str {
    match code {
        "global" => "Global",
        "de" => "Germany",
        "en" => "English",
        "es" => "Spain",
        "fr" => "France",
        "it" => "Italy",
        "ja" => "Japan",
        "pl" => "Poland",
        "ru" => "Russia",
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
    let locale = get_locale_name(name.split_at(split).1);
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
