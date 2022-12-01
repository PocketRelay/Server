use crate::models::stats::{
    EmptyLeaderboardResponse, EntityCountResponse, LeaderboardGroupRequest,
    LeaderboardGroupResponse,
};
use crate::session::Session;
use crate::HandleResult;
use blaze_pk::packet::Packet;
use core::blaze::components::Stats;

/// Routing function for handling packets with the `Stats` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
///
/// `session`   The session that the packet was recieved by
/// `component` The component of thet recieved
/// `packet`    The recieved packet
pub fn route(_session: &mut Session, component: Stats, packet: &Packet) -> HandleResult {
    match component {
        Stats::GetLeaderboardEntityCount => handle_leaderboard_entity_count(packet),
        Stats::GetCenteredLeaderboard => handle_centered_leaderboard(packet),
        Stats::GetFilteredLeaderboard => handle_filtered_leaderboard(packet),
        Stats::GetLeaderboardGroup => handle_leaderboard_group(packet),
        _ => Ok(packet.respond_empty()),
    }
}

/// Handles returning the number of leaderboard objects present.
/// This is currently not implemented
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
fn handle_leaderboard_entity_count(packet: &Packet) -> HandleResult {
    // The total number of players in the database for both responses
    let response = EntityCountResponse { count: 1 };
    Ok(packet.respond(response))
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
fn handle_centered_leaderboard(packet: &Packet) -> HandleResult {
    // 30 - pre
    // me
    // 29 - post

    // leaderboard but where the request CENT player ID is in the center
    // or as close to center as possible for uneven COUN number

    Ok(packet.respond(EmptyLeaderboardResponse))
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
fn handle_filtered_leaderboard(packet: &Packet) -> HandleResult {
    // Leaderboard but only returns self

    Ok(packet.respond(EmptyLeaderboardResponse))
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
