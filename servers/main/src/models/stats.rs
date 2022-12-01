use blaze_pk::{
    codec::{Decodable, Encodable},
    error::DecodeResult,
    reader::TdfReader,
    tag::TdfType,
    writer::TdfWriter,
};

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
