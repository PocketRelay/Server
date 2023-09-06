use tdf::{
    types::var_int::skip_var_int, DecodeError, TdfDeserialize, TdfDeserializeOwned, TdfSerialize,
    TdfType, TdfTyped,
};

use crate::{
    services::leaderboard::models::LeaderboardEntry,
    utils::{components::user_sessions::PLAYER_TYPE, types::PlayerID},
};

/// Structure for the request to retrieve the entity count
/// of a leaderboard
#[derive(TdfDeserialize)]
pub struct EntityCountRequest {
    /// The leaderboard name
    #[tdf(tag = "NAME")]
    pub name: String,
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
    pub count: usize,
    /// The leaderboard name
    #[tdf(tag = "NAME")]
    pub name: String,
}

pub enum LeaderboardResponse<'a> {
    /// Empty response where there is no content
    Empty,
    /// Response with one entry
    One(&'a LeaderboardEntry),
    /// Response with many leaderboard entires
    Many(&'a [LeaderboardEntry]),
}

impl TdfSerialize for LeaderboardEntry {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.group_body(|w| {
            w.tag_str(b"ENAM", &self.player_name);
            w.tag_u32(b"ENID", self.player_id);
            w.tag_usize(b"RANK", self.rank);

            let value_str = self.value.to_string();
            w.tag_str(b"RSTA", &value_str);
            w.tag_zero(b"RWFG");
            w.tag_union_unset(b"RWST");

            w.tag_list_slice(b"STAT", &[value_str]);

            w.tag_zero(b"UATT");
        });
    }
}

impl TdfTyped for LeaderboardEntry {
    const TYPE: TdfType = TdfType::Group;
}

impl TdfSerialize for LeaderboardResponse<'_> {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        match self {
            Self::Empty => {
                w.tag_list_empty(b"LDLS", TdfType::Group);
            }
            Self::One(value) => {
                w.tag_list_start(b"LDLS", TdfType::Group, 1);
                value.serialize(w);
            }
            Self::Many(values) => {
                w.tag_list_slice(b"LDLS", values);
            }
        }
    }
}

/// Structure for the request to retrieve a leaderboards
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
    pub count: usize,
    /// The leaderboard name
    #[tdf(tag = "NAME")]
    pub name: String,
    /// The rank offset to start at
    #[tdf(tag = "STRT")]
    pub start: usize,
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
pub struct FilteredLeaderboardRequest {
    /// The player ID
    pub id: PlayerID,
    /// The leaderboard name
    pub name: String,
}

impl TdfDeserializeOwned for FilteredLeaderboardRequest {
    fn deserialize_owned(r: &mut tdf::TdfDeserializer<'_>) -> tdf::DecodeResult<Self> {
        let count: usize = r.until_list_typed(b"IDLS", TdfType::VarInt)?;
        if count < 1 {
            return Err(DecodeError::Other("Missing player ID for filter"));
        }
        let id: PlayerID = PlayerID::deserialize_owned(r)?;
        for _ in 1..count {
            skip_var_int(r)?;
        }
        let name: String = r.tag(b"NAME")?;
        Ok(Self { id, name })
    }
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
