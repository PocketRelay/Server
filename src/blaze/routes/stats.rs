use crate::blaze::components::Stats;
use crate::blaze::errors::HandleResult;
use crate::blaze::SessionArc;
use blaze_pk::{
    encode_str, packet, tag_group_end, tag_list_start, tag_map_start, tag_pair, tag_str, tag_u32,
    tag_u8, Codec, OpaquePacket, ValueType,
};
use log::debug;

/// Routing function for handling packets with the `Stats` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
pub async fn route(session: &SessionArc, component: Stats, packet: &OpaquePacket) -> HandleResult {
    match component {
        Stats::GetLeaderboardEntityCount => handle_leaderboard_entity_count(session, packet).await,
        Stats::GetCenteredLeaderboard => handle_centered_leaderboard(session, packet).await,
        Stats::GetFilteredLeaderboard => handle_filtered_leaderboard(session, packet).await,
        Stats::GetLeaderboardGroup => handle_leaderboard_group(session, packet).await,
        component => {
            debug!("Got Stats({component:?})");
            packet.debug_decode()?;
            session.response_empty(packet).await
        }
    }
}

struct EntityCount {
    count: u32,
}

impl Codec for EntityCount {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_u32(output, "CNT", self.count);
    }
}

/// Handles returning the number of leaderboard objects present.
/// This is currently not implemented
///
/// # Structure
/// ```
/// packet(Components.STATS, Commands.GET_LEADERBOARD_ENTITY_COUNT, 0x0) {
///   map("KSUM", mapOf(
///     "accountcountry" to 0x0,
///     "ME3Map" to 0x0,
///   ))
///   number("LBID", 0x0)
///   text("NAME", "N7RatingGlobal")
///   number("POFF", 0x0)
/// }
/// ```
async fn handle_leaderboard_entity_count(
    session: &SessionArc,
    packet: &OpaquePacket,
) -> HandleResult {
    session.response(packet, &EntityCount { count: 1 }).await
}

struct EmptyLeaderboard;

impl Codec for EmptyLeaderboard {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_list_start(output, "LDLS", ValueType::Group, 0);
    }
}

/// Handles returning a centered leaderboard object. This is currently not implemented
///
/// # Structure
/// ```
/// packet(Components.STATS, Commands.GET_CENTERED_LEADERBOARD, 0x0) {
///   number("BOTT", 0x0)
///   number("CENT", 0x3a5508eb)
///   number("COUN", 0x3c)
///   map("KSUM", mapOf(
///     "accountcountry" to 0x0,
///     "ME3Map" to 0x0,
///   ))
///   number("LBID", 0x0)
///   text("NAME", "N7RatingGlobal")
///   number("POFF", 0x0)
///   number("TIME", 0x0)
///   tripple("USET", 0x0, 0x0, 0x0)
/// }
/// ```
async fn handle_centered_leaderboard(session: &SessionArc, packet: &OpaquePacket) -> HandleResult {
    session.response(packet, &EmptyLeaderboard).await
}

/// Handles returning a filtered leaderboard object. This is currently not implemented
///
/// # Structure
/// ```
/// packet(Components.STATS, Commands.GET_FILTERED_LEADERBOARD, 0x1b) {
///   number("FILT", 0x1)
///   list("IDLS", listOf(0x3a5508eb))
///   map("KSUM", mapOf(
///     "accountcountry" to 0x0,
///     "ME3Map" to 0x0,
///   ))
///   number("LBID", 0x0)
///   text("NAME", "N7RatingGlobal")
///   number("POFF", 0x0)
///   number("TIME", 0x0)
///   tripple("USET", 0x0, 0x0, 0x0)
/// }
/// ```
async fn handle_filtered_leaderboard(session: &SessionArc, packet: &OpaquePacket) -> HandleResult {
    session.response(packet, &EmptyLeaderboard).await
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

struct LeaderboardGroup<'a> {
    name: String,
    desc: String,
    sname: &'a str,
    sdsc: &'a str,
    gname: &'a str,
}

impl Codec for LeaderboardGroup<'_> {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_u8(output, "ACSD", 0);
        tag_str(output, "BNAM", &self.name);
        tag_str(output, "DESC", &self.desc);
        tag_pair(output, "ETYP", &(0x7802, 0x1));
        {
            tag_map_start(output, "KSUM", ValueType::String, ValueType::Group, 1);
            encode_str("accountcountry", output);
            {
                tag_map_start(output, "KSVL", ValueType::VarInt, ValueType::VarInt, 1);
                output.push(0);
                output.push(0);
                tag_group_end(output);
            }
        }
        tag_u32(output, "LBSZ", 0x7270e0);
        {
            tag_list_start(output, "LIST", ValueType::Group, 1);
            {
                tag_str(output, "CATG", "MassEffectStats");
                tag_str(output, "DFLT", "0");
                tag_u8(output, "DRVD", 0x0);
                tag_str(output, "FRMT", "%d");
                tag_str(output, "KIND", "");
                tag_str(output, "LDSC", self.sdsc);
                tag_str(
                    output,
                    "META",
                    "W=200, HMC=tableColHeader3, REMC=tableRowEntry3",
                );
                tag_str(output, "NAME", self.sname);
                tag_str(output, "SDSC", self.sdsc);
                tag_u8(output, "TYPE", 0x0);
                tag_group_end(output);
            }
        }
        tag_str(output, "META", "RF=@W=150, HMC=tableColHeader1, REMC=tableRowEntry1@ UF=@W=670, HMC=tableColHeader2, REMC=tableRowEntry2@");
        tag_str(output, "NAME", self.gname);
        tag_str(output, "SNAM", self.sname);
    }
}

packet! {
    struct LeaderboardGroupReq {
        NAME name: String,
    }
}

///
///
/// # Structure
/// ```
/// packet(Components.STATS, Commands.GET_LEADERBOARD_GROUP, 0x13) {
///   number("LBID", 0x1)
///   text("NAME", "N7RatingGlobal")
/// }
/// ```
///
async fn handle_leaderboard_group(session: &SessionArc, packet: &OpaquePacket) -> HandleResult {
    let req = packet.contents::<LeaderboardGroupReq>()?;
    let name = req.name;
    let is_n7 = name.starts_with("N7Rating");
    if !is_n7 && !name.starts_with("ChallengePoints") {
        return session.response_empty(packet).await;
    }
    let split = if is_n7 { 8 } else { 15 };
    let locale = get_locale_name(name.split_at(split).1);
    let group = if is_n7 {
        LeaderboardGroup {
            name,
            desc: format!("N7 Rating - {locale}"),
            sname: "n7rating",
            sdsc: "N7 Rating",
            gname: "ME3LeaderboardGroup",
        }
    } else {
        LeaderboardGroup {
            name,
            desc: format!("Challenge Points - {locale}"),
            sname: "ChallengePoints",
            sdsc: "Challenge Points",
            gname: "ME3ChallengePoints",
        }
    };
    session.response(packet, &group).await
}
