use crate::models::stats::{
    CenteredLeaderboardRequest, EmptyLeaderboardResponse, EntityCountRequest, EntityCountResponse,
    FilteredLeaderboardRequest, FilteredLeaderboardResponse, LeaderboardGroupRequest,
    LeaderboardGroupResponse, LeaderboardRequest, LeaderboardResponse,
};
use crate::routes::HandleResult;
use crate::session::Session;
use blaze_pk::packet::Packet;
use core::blaze::components::Stats;
use core::leaderboard::models::{LeaderboardEntityGroup, LeaderboardEntry, LeaderboardType};
use core::state::GlobalState;

/// Routing function for handling packets with the `Stats` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
///
/// `session`   The session that the packet was recieved by
/// `component` The component of thet recieved
/// `packet`    The recieved packet
pub async fn route(_session: &mut Session, component: Stats, packet: &Packet) -> HandleResult {
    match component {
        Stats::GetLeaderboardEntityCount => handle_leaderboard_entity_count(packet).await,
        Stats::GetLeaderboard => handle_leaderboard(packet).await,
        Stats::GetCenteredLeaderboard => handle_centered_leaderboard(packet).await,
        Stats::GetFilteredLeaderboard => handle_filtered_leaderboard(packet).await,
        Stats::GetLeaderboardGroup => handle_leaderboard_group(packet),
        _ => Ok(packet.respond_empty()),
    }
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
async fn handle_leaderboard_entity_count(packet: &Packet) -> HandleResult {
    let request: EntityCountRequest = packet.decode()?;
    let leaderboard = GlobalState::leaderboard();
    let ty = LeaderboardType::from(request.name);
    let (count, _) = leaderboard.get(ty).await?;
    let response = EntityCountResponse { count };
    Ok(packet.respond(response))
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
async fn handle_leaderboard(packet: &Packet) -> HandleResult {
    let request: LeaderboardRequest = packet.decode()?;
    // Leaderboard but only returns self
    let leaderboard = GlobalState::leaderboard();
    let ty = LeaderboardType::from(request.name);
    let (_, group) = leaderboard.get(ty).await?;
    let group = &*group.read().await;

    let start_index = request.start;
    let end_index = request.count.min(group.values.len());

    let values: Option<&[LeaderboardEntry]> = group.values.get(start_index..end_index);
    if let Some(values) = values {
        Ok(packet.respond(LeaderboardResponse { values }))
    } else {
        return Ok(packet.respond(EmptyLeaderboardResponse));
    }
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
async fn handle_centered_leaderboard(packet: &Packet) -> HandleResult {
    let request: CenteredLeaderboardRequest = packet.decode()?;
    let count = request.count.max(1);
    let before = if count % 2 == 0 {
        count / 2 + 1
    } else {
        count / 2
    };
    let after = count / 2;

    let leaderboard = GlobalState::leaderboard();
    let ty = LeaderboardType::from(request.name);
    let (_, group) = leaderboard.get(ty).await?;
    let group: &LeaderboardEntityGroup = &*group.read().await;

    let index_of = group
        .values
        .iter()
        .position(|value| value.player_id == request.center);

    let index_of = match index_of {
        Some(value) => value,
        None => return Ok(packet.respond(EmptyLeaderboardResponse)),
    };

    let start_index = index_of - before.min(index_of);
    let end_index = (index_of + after).min(group.values.len());

    let values: Option<&[LeaderboardEntry]> = group.values.get(start_index..end_index);
    if let Some(values) = values {
        Ok(packet.respond(LeaderboardResponse { values }))
    } else {
        return Ok(packet.respond(EmptyLeaderboardResponse));
    }
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
async fn handle_filtered_leaderboard(packet: &Packet) -> HandleResult {
    let request: FilteredLeaderboardRequest = packet.decode()?;
    let player_id = request.id;
    // Leaderboard but only returns self
    let leaderboard = GlobalState::leaderboard();
    let ty = LeaderboardType::from(request.name);
    let (_, group) = leaderboard.get(ty).await?;
    let group: &LeaderboardEntityGroup = &*group.read().await;
    let entry = group
        .values
        .iter()
        .find(|value| value.player_id == player_id);

    if let Some(entry) = entry {
        Ok(packet.respond(FilteredLeaderboardResponse { value: entry }))
    } else {
        Ok(packet.respond(EmptyLeaderboardResponse))
    }
}

fn get_locale_name(code: &str) -> String {
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
    .to_string()
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
fn handle_leaderboard_group(packet: &Packet) -> HandleResult {
    let req: LeaderboardGroupRequest = packet.decode()?;
    let name = req.name;
    let is_n7 = name.starts_with("N7Rating");
    if !is_n7 && !name.starts_with("ChallengePoints") {
        return Ok(packet.respond_empty());
    }
    let split = if is_n7 { 8 } else { 15 };
    let locale = get_locale_name(name.split_at(split).1);
    let group = if is_n7 {
        LeaderboardGroupResponse {
            name,
            desc: format!("N7 Rating - {locale}"),
            sname: "n7rating",
            sdsc: "N7 Rating",
            gname: "ME3LeaderboardGroup",
        }
    } else {
        LeaderboardGroupResponse {
            name,
            desc: format!("Challenge Points - {locale}"),
            sname: "ChallengePoints",
            sdsc: "Challenge Points",
            gname: "ME3ChallengePoints",
        }
    };
    Ok(packet.respond(group))
}
