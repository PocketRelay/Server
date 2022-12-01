use core::leaderboard::models::LeaderboardEntry;

use blaze_pk::{
    codec::{Decodable, Encodable},
    error::{DecodeError, DecodeResult},
    reader::TdfReader,
    tag::TdfType,
    writer::TdfWriter,
};
use utils::types::PlayerID;

/// Structure for the request to retrieve the entity count
/// of a leaderboard
pub struct EntityCountRequest {
    /// The leaderboard name
    pub name: String,
}

impl Decodable for EntityCountRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let name: String = reader.tag("NAME")?;
        Ok(Self { name })
    }
}

/// Structure for the entity count response for finding the
/// number of entities in a leaderboard section
pub struct EntityCountResponse {
    /// The number of entities in the leaderboard
    pub count: usize,
}

impl Encodable for EntityCountResponse {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_usize(b"CNT", self.count);
    }
}

fn write_leaderboard_entry(writer: &mut TdfWriter, value: &LeaderboardEntry) {
    writer.tag_str(b"ENAM", &value.player_name);
    writer.tag_u32(b"ENID", value.player_id);
    writer.tag_usize(b"RANK", value.rank);
    let value_str = value.value.to_string();
    writer.tag_str(b"RSTA", &value_str);
    writer.tag_zero(b"RWFG");
    writer.tag_union_unset(b"RWST");
    {
        writer.tag_list_start(b"STAT", TdfType::String, 1);
        writer.write_str(&value_str);
    }
    writer.tag_zero(b"UATT");
    writer.tag_group_end();
}

pub struct CenteredLeaderboardRequest {
    /// The entity count
    pub count: usize,
    /// The leaderboard name
    pub name: String,
    /// The ID of the player to center on
    pub center: PlayerID,
}

impl Decodable for CenteredLeaderboardRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let center: PlayerID = reader.tag("CENT")?;
        let count: usize = reader.tag("COUN")?;
        let name: String = reader.tag("NAME")?;
        Ok(Self {
            center,
            count,
            name,
        })
    }
}

pub struct LeaderboardResponse<'a> {
    pub values: &'a [LeaderboardEntry],
}

impl Encodable for LeaderboardResponse<'_> {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_list_start(b"LDLS", TdfType::Group, self.values.len());
        let mut iter = self.values.iter();
        while let Some(value) = iter.next() {
            write_leaderboard_entry(writer, value)
        }
    }
}

/// Structure for the request to retrieve a leaderboards
/// contents at the provided start offset
pub struct LeaderboardRequest {
    /// The entity count
    pub count: usize,
    /// The leaderboard name
    pub name: String,
    /// The rank offset to start at
    pub start: usize,
}

impl Decodable for LeaderboardRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let count: usize = reader.tag("COUN")?;
        let name: String = reader.tag("NAME")?;
        let start: usize = reader.tag("STRT")?;
        Ok(Self { count, name, start })
    }
}

/// Structure for a request to get a leaderboard only
/// containing the details for a specific player
pub struct FilteredLeaderboardRequest {
    /// The player ID
    pub id: PlayerID,
    /// The leaderboard name
    pub name: String,
}

impl Decodable for FilteredLeaderboardRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let count: usize = reader.until_list("IDLS", TdfType::VarInt)?;
        if count < 1 {
            return Err(DecodeError::Other("Missing player ID for filter"));
        }
        let id: PlayerID = reader.read_u32()?;
        for _ in 1..count {
            reader.skip_var_int();
        }
        let name: String = reader.tag("NAME")?;
        Ok(Self { id, name })
    }
}

pub struct FilteredLeaderboardResponse<'a> {
    pub value: &'a LeaderboardEntry,
}

impl Encodable for FilteredLeaderboardResponse<'_> {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_list_start(b"LDLS", TdfType::Group, 1);
        write_leaderboard_entry(writer, self.value)
    }
}

/// Structure for an empty leaderboard response
///
/// # Example
///
/// ```
/// Content: {
///  "LDLS": List<Group> [
///    {
///      "ENAM": "Jacobtread",
///      "ENID": 978651371, PLAYER ID
///      "RANK": 45, Leaderboard rank value
///      "RSTA": "91920",
///      "RWFG": 0,
///      "RWST": Optional(Empty),
///      "STAT": List<String> ["91920"],
///      "UATT": 0,
///    }
///  ],
///}
/// ```
pub struct EmptyLeaderboardResponse;

impl Encodable for EmptyLeaderboardResponse {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_list_start(b"LDLS", TdfType::Group, 0);
    }
}

/// Structure for a request for a leaderboard group
pub struct LeaderboardGroupRequest {
    /// The name of the leaderboard group
    pub name: String,
}

impl Decodable for LeaderboardGroupRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let name: String = reader.tag("NAME")?;
        Ok(Self { name })
    }
}

/// Structure for a leaderboard group response.
pub struct LeaderboardGroupResponse<'a> {
    pub name: String,
    pub desc: String,
    pub sname: &'a str,
    pub sdsc: &'a str,
    pub gname: &'a str,
}

impl Encodable for LeaderboardGroupResponse<'_> {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u8(b"ACSD", 0);
        writer.tag_str(b"BNAM", &self.name);
        writer.tag_str(b"DESC", &self.desc);
        writer.tag_pair(b"ETYP", (0x7802, 0x1));
        {
            writer.tag_map_start(b"KSUM", TdfType::String, TdfType::Group, 1);
            writer.write_str("accountcountry");
            {
                writer.tag_map_start(b"KSVL", TdfType::VarInt, TdfType::VarInt, 1);
                writer.write_byte(0);
                writer.write_byte(0);
                writer.tag_group_end();
            }
        }
        writer.tag_u32(b"LBSZ", 0x7270e0);
        {
            writer.tag_list_start(b"LIST", TdfType::Group, 1);
            {
                writer.tag_str(b"CATG", "MassEffectStats");
                writer.tag_str(b"DFLT", "0");
                writer.tag_u8(b"DRVD", 0x0);
                writer.tag_str(b"FRMT", "%d");
                writer.tag_str(b"KIND", "");
                writer.tag_str(b"LDSC", self.sdsc);
                writer.tag_str(b"META", "W=200, HMC=tableColHeader3, REMC=tableRowEntry3");
                writer.tag_str(b"NAME", self.sname);
                writer.tag_str(b"SDSC", self.sdsc);
                writer.tag_u8(b"TYPE", 0x0);
                writer.tag_group_end();
            }
        }
        writer. tag_str(b"META", "RF=@W=150, HMC=tableColHeader1, REMC=tableRowEntry1@ UF=@W=670, HMC=tableColHeader2, REMC=tableRowEntry2@");
        writer.tag_str(b"NAME", self.gname);
        writer.tag_str(b"SNAM", self.sname);
    }
}
