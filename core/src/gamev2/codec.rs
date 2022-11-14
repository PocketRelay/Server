use blaze_pk::{codec::Codec, packet, tagging::*};
use database::players;

use crate::blaze::session::SessionData;

use super::game::AttrMap;

packet! {
    // Packet for game state changes
    struct StateChange {
        // The id of the game the state has changed for
        GID id: u32,
        // The new state value
        GSTA state: u16
    }
}

packet! {
    // Packet for game setting changes
    struct SettingChange {
        // The new setting value
        ATTR setting: u16,
        // The id of the game the setting has changed for
        GID id: u32,
    }
}

/// Packet for game attribute changes
pub struct AttributesChange<'a> {
    /// The id of the game the attributes have changed for
    pub id: u32,
    /// Borrowed game attributes map
    pub attributes: &'a AttrMap,
}

impl Codec for AttributesChange<'_> {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_value(output, "ATTR", self.attributes);
        tag_u32(output, "GID", self.id);
    }
}

/// Encodes the provided players data to the provided output
///
/// `session_data` The session data to encode
/// `player`       The player attached to the session
/// `game_id`      The game the session is in
/// `slot`         The slot in the game the session is in
/// `output`       The output to encode to
fn encode_player_data(
    session_data: &SessionData,
    player: &players::Model,
    game_id: u32,
    slot: usize,
    output: &mut Vec<u8>,
) {
    tag_empty_blob(output, "BLOB");
    tag_u8(output, "EXID", 0);
    tag_u32(output, "GID", game_id);
    tag_u32(output, "LOC", 0x64654445);
    tag_str(output, "NAME", &player.display_name);
    let player_id = session_data.id_safe();
    tag_u32(output, "PID", player_id);
    tag_value(output, "PNET", &session_data.net.get_groups());
    tag_usize(output, "SID", slot);
    tag_u8(output, "SLOT", 0);
    tag_u8(output, "STAT", session_data.state);
    tag_u16(output, "TIDX", 0xffff);
    tag_u8(output, "TIME", 0);
    tag_triple(output, "UGID", &(0, 0, 0));
    tag_u32(output, "UID", player_id);
    tag_group_end(output);
}

pub struct PlayerJoining<'a> {
    /// The player ID of the joining player
    pub id: u32,
    /// The slot the player is joining into
    pub slot: usize,
    /// The session of the player that is joining
    pub session: &'a SessionData,
}

impl Codec for PlayerJoining<'_> {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_u32(output, "GID", self.id);

        if let Some(player) = self.session.player.as_ref() {
            tag_group_start(output, "PDAT");
            encode_player_data(self.session, player, self.id, self.slot, output);
        }
    }
}
