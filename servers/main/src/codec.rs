use blaze_pk::{codec::Codec, tag::ValueType, tagging::*};
use utils::types::PlayerID;

use crate::session::Session;

fn encode_session(session: &Session, output: &mut Vec<u8>) {
    tag_value(output, "ADDR", &session.net.get_groups());
    tag_str(output, "BPS", "ea-sjc");
    tag_empty_str(output, "CTY");
    tag_var_int_list_empty(output, "CVAR");
    {
        tag_map_start(output, "DMAP", ValueType::VarInt, ValueType::VarInt, 1);
        0x70001.encode(output);
        0x409a.encode(output);
    }
    tag_u16(output, "HWFG", session.net.hwfg);
    {
        tag_list_start(output, "PSLM", ValueType::VarInt, 1);
        0xfff0fff.encode(output);
    }
    tag_value(output, "QDAT", &session.net.ext);
    tag_u8(output, "UATT", 0);
    if let Some(game_id) = &session.game {
        tag_list_start(output, "ULST", ValueType::Triple, 1);
        (4, 1, *game_id).encode(output);
    }
    tag_group_end(output);
}

/// Session update for a session other than ourselves
/// which contains the details for that session
pub struct SessionUpdate<'a> {
    pub session: &'a Session,
    pub player_id: PlayerID,
    pub display_name: &'a str,
}

impl Codec for SessionUpdate<'_> {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_group_start(output, "DATA");
        encode_session(self.session, output);

        tag_group_start(output, "USER");
        tag_u32(output, "AID", self.player_id);
        tag_u32(output, "ALOC", 0x64654445);
        tag_empty_blob(output, "EXBB");
        tag_u8(output, "EXID", 0);
        tag_u32(output, "ID", self.player_id);
        tag_str(output, "NAME", self.display_name);
        tag_group_end(output);
    }
}

/// Session update for ourselves
pub struct SetSession<'a> {
    pub player_id: PlayerID,
    pub session: &'a Session,
}

impl Codec for SetSession<'_> {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_group_start(output, "DATA");
        encode_session(self.session, output);
        tag_u32(output, "USID", self.player_id);
    }
}
