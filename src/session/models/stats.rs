use crate::{
    database::entities::leaderboard_data::{LeaderboardDataAndRank, LeaderboardType},
    utils::{components::user_sessions::PLAYER_TYPE, types::PlayerID},
};
use tdf::{TdfDeserialize, TdfMap, TdfSerialize, TdfType, TdfTyped, VarIntList};

#[derive(TdfDeserialize)]
pub struct SubmitGameReportRequest {
    #[tdf(tag = "RPRT")]
    pub report: GameReport,
}

#[derive(TdfDeserialize, TdfTyped)]
#[tdf(group)]
pub struct GameReport {
    // Must be read since it uses the same duplicate tag
    #[tdf(tag = "GAME")]
    #[allow(unused)]
    pub game_ids: VarIntList,

    #[tdf(tag = "GAME")]
    pub game: GameReportGame,
}

#[derive(TdfDeserialize, TdfTyped)]
#[tdf(group)]
pub struct GameReportGame {
    /// The details for each specific player
    #[tdf(tag = "PLYR")]
    pub players: TdfMap<PlayerID, GameReportPlayerData>,
}

#[derive(TdfDeserialize, TdfTyped)]
#[tdf(group)]
pub struct GameReportPlayerData {
    /// Locale string encoded as int
    #[tdf(tag = "CTRY")]
    #[allow(unused)]
    pub country: u32,
    /// Number of challenge points the player has
    #[tdf(tag = "NCHP")]
    pub challenge_points: u32,
    /// N7 Rating value for the player
    #[tdf(tag = "NRAT")]
    pub n7_rating: u32,
}

#[test]
fn test() {
    let bytes = 17477u32.to_be_bytes();
    println!("{}", String::from_utf8_lossy(&bytes));
}

/// Structure for the request to retrieve the entity count
/// of a leaderboard
#[derive(TdfDeserialize)]
pub struct EntityCountRequest {
    /// The leaderboard name
    #[tdf(tag = "NAME", into = &str)]
    pub name: LeaderboardType,
}

/// Structure for the entity count response for finding the
/// number of entities in a leaderboard section
#[derive(TdfSerialize)]
pub struct EntityCountResponse {
    /// The number of entities in the leaderboard
    #[tdf(tag = "CNT")]
    pub count: usize,
}

/// Request for a list of leaderboard entries where the center
/// value is the entry for the player with the provided ID
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
#[derive(TdfDeserialize)]
pub struct CenteredLeaderboardRequest {
    /// The ID of the player to center on
    #[tdf(tag = "CENT")]
    pub center: PlayerID,
    /// The entity count
    #[tdf(tag = "COUN")]
    pub count: u32,
    /// The leaderboard name
    #[tdf(tag = "NAME", into = &str)]
    pub name: LeaderboardType,
}

pub struct LeaderboardResponse {
    pub values: Vec<LeaderboardDataAndRank>,
}

impl TdfSerialize for LeaderboardDataAndRank {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.group_body(|w| {
            w.tag_str(b"ENAM", &self.player_name);
            w.tag_u32(b"ENID", self.player_id);
            w.tag_u32(b"RANK", self.rank);

            let value_str = self.value.to_string();
            w.tag_str(b"RSTA", &value_str);
            w.tag_zero(b"RWFG");
            w.tag_union_unset(b"RWST");

            w.tag_list_slice(b"STAT", &[value_str]);

            w.tag_zero(b"UATT");
        });
    }
}

impl TdfTyped for LeaderboardDataAndRank {
    const TYPE: TdfType = TdfType::Group;
}

impl TdfSerialize for LeaderboardResponse {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.tag_list_slice(b"LDLS", &self.values);
    }
}

/// Structure for the request to retrieve a leaderboard
/// contents at the provided start offset
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
#[derive(TdfDeserialize)]
pub struct LeaderboardRequest {
    /// The entity count
    #[tdf(tag = "COUN")]
    pub count: u32,
    /// The leaderboard name
    #[tdf(tag = "NAME", into = &str)]
    pub name: LeaderboardType,
    /// The rank offset to start at
    #[tdf(tag = "STRT")]
    pub start: u32,
}

/// Structure for a request to get a leaderboard only
/// containing the details for a specific player
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
#[derive(TdfDeserialize)]
pub struct FilteredLeaderboardRequest {
    /// The player ID
    #[tdf(tag = "IDLS")]
    pub ids: Vec<PlayerID>,
    /// The leaderboard name
    #[tdf(tag = "NAME", into = &str)]
    pub name: LeaderboardType,
}

/// Structure for a request for a leaderboard group
#[derive(TdfDeserialize)]
pub struct LeaderboardGroupRequest {
    /// The name of the leaderboard group
    #[tdf(tag = "NAME")]
    pub name: String,
}

/// Structure for a leaderboard group response.
pub struct LeaderboardGroupResponse<'a> {
    pub name: String,
    pub desc: String,
    pub sname: &'a str,
    pub sdsc: &'a str,
    pub gname: &'a str,
}

impl TdfSerialize for LeaderboardGroupResponse<'_> {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.tag_u8(b"ACSD", 0);
        w.tag_str(b"BNAM", &self.name);
        w.tag_str(b"DESC", &self.desc);
        w.tag_alt(b"ETYP", PLAYER_TYPE);

        {
            w.tag_map_start(b"KSUM", TdfType::String, TdfType::Group, 1);
            "accountcountry".serialize(w);
            w.group_body(|w| {
                w.tag_map_tuples(b"KSVL", &[(0u8, 0u8)]);
            });
        }
        w.tag_u32(b"LBSZ", 0x7270e0);
        {
            w.tag_list_start(b"LIST", TdfType::Group, 1);
            w.group_body(|w| {
                w.tag_str(b"CATG", "MassEffectStats");
                w.tag_str(b"DFLT", "0");
                w.tag_u8(b"DRVD", 0x0);
                w.tag_str(b"FRMT", "%d");
                w.tag_str(b"KIND", "");
                w.tag_str(b"LDSC", self.sdsc);
                w.tag_str(b"META", "W=200, HMC=tableColHeader3, REMC=tableRowEntry3");
                w.tag_str(b"NAME", self.sname);
                w.tag_str(b"SDSC", self.sdsc);
                w.tag_u8(b"TYPE", 0x0);
            });
        }
        w.tag_str(b"META", "RF=@W=150, HMC=tableColHeader1, REMC=tableRowEntry1@ UF=@W=670, HMC=tableColHeader2, REMC=tableRowEntry2@");
        w.tag_str(b"NAME", self.gname);
        w.tag_str(b"SNAM", self.sname);
    }
}
