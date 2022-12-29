use crate::{
    blaze::{
        components::{Components as C, Stats as S},
        errors::BlazeResult,
    },
    leaderboard::models::*,
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
    router.route(C::Stats(S::GetLeaderboard), handle_leaderboard);
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
    let (count, _) = leaderboard.get(ty).await?;
    Ok(EntityCountResponse { count })
}

///
///
/// Component: Stats(GetLeaderboard)
/// ```
/// ID: 1274
/// Content: {
///   "COUN": 61,
///   "KSUM": Map {
///     "accountcountry": 0
///     "ME3Map": 0
///   },
///   "LBID": 0,
///   "NAME": "N7RatingGlobal",
///   "POFF": 0,
///   "STRT": 29,
///   "TIME": 0,
///   "USET": (0, 0, 0),
/// }
/// ```
async fn handle_leaderboard(req: LeaderboardRequest) -> BlazeResult<()> {
    // Leaderboard but only returns self
    let leaderboard = GlobalState::leaderboard();
    let ty = LeaderboardType::from(req.name);
    let (_, group) = leaderboard.get(ty).await?;
    let group = &*group.read().await;

    let start_index = req.start;
    let end_index = req.count.min(group.values.len());

    let values: Option<&[LeaderboardEntry]> = group.values.get(start_index..end_index);
    // TODO: IMEPLEMENT PROPERLY
    // Ok(LeaderboardResponse { values })
    Ok(())
}

/// Handles returning a centered leaderboard object. This is currently not implemented
///
/// ```
/// Route: Stats(GetCenteredLeaderboard)
/// ID: 0
/// Content: {
///     "BOTT": 0,
///     "CENT": 1, // Player ID to center on
///     "COUN": 60,
///     "KSUM": Map {
///         "accountcountry": 0,
///         "ME3Map": 0
///     },
///     "LBID": 0,
///     "NAME": "N7RatingGlobal",
///     "POFF": 0,
///     "TIME": 0,
///     "USET": (0, 0, 0)
/// }
/// ```
async fn handle_centered_leaderboard(req: CenteredLeaderboardRequest) -> BlazeResult<()> {
    let count = req.count.max(1);
    let before = if count % 2 == 0 {
        count / 2 + 1
    } else {
        count / 2
    };
    let after = count / 2;

    let leaderboard = GlobalState::leaderboard();
    let ty = LeaderboardType::from(req.name);
    let (_, group) = leaderboard.get(ty).await?;
    let group: &LeaderboardEntityGroup = &*group.read().await;

    let index_of = group
        .values
        .iter()
        .position(|value| value.player_id == req.center);

    let index_of = match index_of {
        Some(value) => value,
        // None => return Ok(LeaderboardResponse { values: &[] }),
        None => return Ok(()),
    };

    let start_index = index_of - before.min(index_of);
    let end_index = (index_of + after).min(group.values.len());

    let values: Option<&[LeaderboardEntry]> = group.values.get(start_index..end_index);
    Ok(())

    // TODO: IMPLEMENT PROPERLY
    // Ok(LeaderboardResponse { values })
}

/// Handles returning a filtered leaderboard object. This is currently not implemented
///
/// ```
/// Route: Stats(GetFilteredLeaderboard)
/// ID: 27
/// Content: {
///     "FILT": 1,
///     "IDLS": [1], // Player IDs
///     "KSUM": Map {
///         "accountcountry": 0,
///         "ME3Map": 0
///     },
///     "LBID": 0,
///     "NAME": "N7RatingGlobal",
///     "POFF": 0,
///     "TIME": 0,
///     "USET": (0, 0, 0)
/// }
/// ```
async fn handle_filtered_leaderboard(req: FilteredLeaderboardRequest) -> BlazeResult<()> {
    let player_id = req.id;
    // Leaderboard but only returns self
    let leaderboard = GlobalState::leaderboard();
    let ty = LeaderboardType::from(req.name);
    let (_, group) = leaderboard.get(ty).await?;
    let group: &LeaderboardEntityGroup = &*group.read().await;
    let entry = group
        .values
        .iter()
        .find(|value| value.player_id == player_id);

    // todo!("Properly implement with response types")

    Ok(())

    // Ok(if let Some(entry) = entry {
    //     packet.respond(FilteredLeaderboardResponse { value: entry })
    // } else {
    //     packet.respond(EmptyLeaderboardResponse)
    // })
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
