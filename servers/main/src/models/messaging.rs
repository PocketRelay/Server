use core::blaze::components::{Components, UserSessions};

use blaze_pk::{
    codec::Codec, packet::PacketComponents, tag::ValueType, tagging::*, types::encode_str,
};
use utils::types::PlayerID;

/// Structure of the response to a fetch messages request. Which tells
/// the client how many messages to expect
pub struct FetchMessageResponse {
    /// The total number of messages to expect
    pub count: usize,
}

impl Codec for FetchMessageResponse {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_usize(output, "MCNT", self.count);
    }
}

/// Structure of a message notification packet
pub struct MessageNotify {
    /// The ID of the player the message is for
    pub player_id: PlayerID,
    /// The message contents
    pub message: String,
}

impl Codec for MessageNotify {
    fn encode(&self, output: &mut Vec<u8>) {
        let ref_value = Components::UserSessions(UserSessions::SetSession).values();
        let player_ref = (ref_value.0, ref_value.1, self.player_id);

        tag_u8(output, "FLAG", 0x1);
        tag_u8(output, "MGID", 0x1);
        tag_str(output, "NAME", &self.message);
        {
            tag_group_start(output, "PYLD");
            {
                tag_map_start(output, "ATTR", ValueType::String, ValueType::String, 1);
                encode_str("B0000", output);
                encode_str("160", output);
            }
            tag_u8(output, "FLAG", 0x1);
            tag_u8(output, "STAT", 0x0);
            tag_u8(output, "TAG", 0x0);
            tag_triple(output, "TARG", &player_ref);
            tag_u8(output, "TYPE", 0x0);
            tag_group_end(output);
        }
        tag_triple(output, "SRCE", &player_ref);
        tag_zero(output, "TIME");
    }
}
