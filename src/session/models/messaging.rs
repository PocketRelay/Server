use crate::utils::{components::user_sessions::PLAYER_TYPE, types::PlayerID};
use tdf::{ObjectId, TdfSerialize};

/// Structure of the response to a fetch messages request. Which tells
/// the client how many messages to expect
#[derive(TdfSerialize)]
pub struct FetchMessageResponse {
    /// The total number of messages to expect
    #[tdf(tag = "MCNT")]
    pub count: usize,
}

/// Structure of a message notification packet
pub struct MessageNotify {
    /// The ID of the player the message is for
    pub player_id: PlayerID,
    /// The message contents
    pub message: String,
}

impl TdfSerialize for MessageNotify {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        let player_ref = ObjectId::new(PLAYER_TYPE, self.player_id as u64);

        w.tag_u8(b"FLAG", 0x1);
        w.tag_u8(b"MGID", 0x1);
        w.tag_str(b"NAME", &self.message);

        w.group(b"PYLD", |w| {
            w.tag_map_tuples(b"ATTR", &[("B0000", "160")]);
            w.tag_u8(b"FLAG", 0x1);
            w.tag_u8(b"STAT", 0x0);
            w.tag_u8(b"TAG", 0x0);
            w.tag_ref(b"TARG", &player_ref);
            w.tag_u8(b"TYPE", 0x0);
        });

        w.tag_ref(b"SRCE", &player_ref);
        w.tag_zero(b"TIME");
    }
}
