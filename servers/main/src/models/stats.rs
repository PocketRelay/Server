use blaze_pk::{
    codec::{Codec, CodecResult, Reader},
    tag::ValueType,
    tagging::*,
    types::encode_str,
};

/// Structure for the entity count response for finding the
/// number of entities in a leaderboard section
pub struct EntityCountResponse {
    pub count: usize,
}

impl Codec for EntityCountResponse {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_usize(output, "CNT", self.count);
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

impl Codec for EmptyLeaderboardResponse {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_list_start(output, "LDLS", ValueType::Group, 0);
    }
}

/// Structure for a request for a leaderboard group
pub struct LeaderboardGroupRequest {
    /// The name of the leaderboard group
    pub name: String,
}

impl Codec for LeaderboardGroupRequest {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let name = expect_tag(reader, "NAME")?;
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

impl Codec for LeaderboardGroupResponse<'_> {
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
