use crate::utils::{
    components::{Components, UserSessions},
    types::PlayerID,
};
use blaze_pk::{codec::Encodable, packet::PacketComponents, tag::TdfType, writer::TdfWriter};

/// Structure of the response to a fetch messages request. Which tells
/// the client how many messages to expect
pub struct FetchMessageResponse {
    /// The total number of messages to expect
    pub count: usize,
}

impl Encodable for FetchMessageResponse {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_usize(b"MCNT", self.count);
    }
}

/// Structure of a message notification packet
pub struct MessageNotify {
    /// The ID of the player the message is for
    pub player_id: PlayerID,
    /// The message contents
    pub message: String,
}

impl Encodable for MessageNotify {
    fn encode(&self, writer: &mut TdfWriter) {
        let ref_value: (u16, u16) = Components::UserSessions(UserSessions::SetSession).values();
        let player_ref: (u16, u16, u32) = (ref_value.0, ref_value.1, self.player_id);

        writer.tag_u8(b"FLAG", 0x1);
        writer.tag_u8(b"MGID", 0x1);
        writer.tag_str(b"NAME", &self.message);
        {
            writer.tag_group(b"PYLD");
            {
                writer.tag_map_start(b"ATTR", TdfType::String, TdfType::String, 1);
                writer.write_str("B0000");
                writer.write_str("160");
            }
            writer.tag_u8(b"FLAG", 0x1);
            writer.tag_u8(b"STAT", 0x0);
            writer.tag_u8(b"TAG", 0x0);
            writer.tag_value(b"TARG", &player_ref);
            writer.tag_u8(b"TYPE", 0x0);
            writer.tag_group_end();
        }
        writer.tag_value(b"SRCE", &player_ref);
        writer.tag_zero(b"TIME");
    }
}
