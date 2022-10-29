use blaze_pk::{Codec, tag_empty_blob, tag_group_end, tag_group_start, tag_str, tag_triple, tag_u16, tag_u32, tag_u8, tag_usize, tag_value};
use crate::blaze::SessionData;

#[derive(Debug)]
pub struct NotifyPlayerJoining<'a> {
    /// ID of the game that the player is joining
    pub id: u32,
    /// The session data of the player that is joining
    pub session: &'a SessionData,
}

impl Codec for NotifyPlayerJoining {
    fn encode(&self, output: &mut Vec<u8>) {

        tag_u32(output, "GID", self.id);
        encode_player_data(self.session, output);
    }
}

pub fn encode_player_data(session: &SessionData, output: &mut Vec<u8>) {
    tag_group_start(output, "PDAT");
    tag_empty_blob(output, "BLOB");
    tag_u8(output, "EXID", 0);
    tag_u32(output, "GID", session.game_id_safe());
    tag_u32(output, "LOC", session.location);
    tag_str(output, "NAME", &session.player_name_safe());
    let player_id = session.player_id_safe();
    tag_u32(output, "PID", player_id);
    tag_value(output, "PNET", &session.net.get_groups());
    tag_usize(output, "SID", session.game_slot_safe());
    tag_u8(output, "STAT", session.state);
    tag_u16(output, "TIDX", 0xffff);
    tag_u8(output, "TIME", 0);
    tag_triple(output, "UGID", &(0, 0, 0));
    tag_u32(output, "UID", player_id);
    tag_group_end(output);
}