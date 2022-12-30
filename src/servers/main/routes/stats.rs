use crate::{
    blaze::{
        components::{Components as C, Stats as S},
        errors::{BlazeResult, ServerError, ServerResult},
    },
    leaderboard::{models::*, LeaderboardQuery},
    servers::main::{models::stats::*, session::SessionAddr},
    state::GlobalState,
};
use blaze_pk::router::Router;

/// Routing function for adding all the routes in this file to the
/// provided router
///
/// `router` The router to add to
pub fn route(router: &mut Router<C, SessionAddr>) {
    router.route(
        C::Stats(S::GetLeaderboardEntityCount),
        handle_leaderboard_entity_count,
    );
    router.route(C::Stats(S::GetLeaderboard), |req: LeaderboardRequest| {
        handle_leaderboard_query(
            req.name,
            LeaderboardQuery::Normal {
                start: req.start,
                count: req.count,
            },
        )
    });
    router.route(
        C::Stats(S::GetCenteredLeaderboard),
        |req: CenteredLeaderboardRequest| {
            handle_leaderboard_query(
                req.name,
                LeaderboardQuery::Centered {
                    player_id: req.center,
                    count: req.count,
                },
            )
        },
    );
    router.route(
        C::Stats(S::GetFilteredLeaderboard),
        |req: FilteredLeaderboardRequest| {
            handle_leaderboard_query(req.name, LeaderboardQuery::Filtered { player_id: req.id })
        },
    );
    router.route(C::Stats(S::GetLeaderboardGroup), handle_leaderboard_group);
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
) -> BlazeResult<EntityCountResponse> {
    let leaderboard = GlobalState::leaderboard();
    let ty = LeaderboardType::from(req.name);
    let count = leaderboard.get_size(ty).await?;
    Ok(EntityCountResponse { count })
}

/// Handler function for handling leaderboard querys and returning the resulting
/// leaderboard
///
/// `name`  The name of the leaderboard
/// `query` The query to resolve
async fn handle_leaderboard_query(
    name: String,
    query: LeaderboardQuery,
) -> ServerResult<LeaderboardResponse> {
    let leaderboard = GlobalState::leaderboard();
    let ty = LeaderboardType::from(name);
    let values = leaderboard
        .get(ty, query)
        .await
        .map_err(|_| ServerError::ServerUnavailable)?;
    let response = match values {
        Some(values) => LeaderboardResponse::Values(values.0),
        None => LeaderboardResponse::Empty,
    };
    Ok(response)
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
