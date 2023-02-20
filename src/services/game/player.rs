use super::models::PlayerState;
use crate::{
    servers::main::session::{SessionLink, SetGameMessage},
    utils::{
        components::{Components, UserSessions},
        models::NetData,
        types::{GameID, PlayerID, SessionID},
    },
};
use blaze_pk::{codec::Encodable, packet::Packet, tag::TdfType, writer::TdfWriter};
use database::Player;
use serde::Serialize;

pub struct GamePlayer {
    /// ID of the session associated to this player
    pub session_id: SessionID,
    /// ID of the game this player is apart of
    pub game_id: GameID,
    /// Session player
    pub player: Player,
    /// Session address
    pub addr: SessionLink,
    /// Networking information for the player
    pub net: NetData,
    /// State of the game player
    pub state: PlayerState,
}

/// Structure for taking a snapshot of the players current
/// state.

#[derive(Serialize)]
pub struct GamePlayerSnapshot {
    pub session_id: SessionID,
    pub player_id: PlayerID,
    pub display_name: String,
    pub net: NetData,
}

impl GamePlayer {
    /// Creates a new game player structure with the provided player
    /// details
    ///
    /// `player` The session player
    /// `net`    The player networking details
    /// `addr`   The session address
    pub fn new(session_id: SessionID, player: Player, net: NetData, addr: SessionLink) -> Self {
        Self {
            session_id,
            player,
            addr,
            net,
            game_id: 1,
            state: PlayerState::Connecting,
        }
    }

    /// Takes a snapshot of the current player state
    /// for serialization
    pub fn snapshot(&self) -> GamePlayerSnapshot {
        GamePlayerSnapshot {
            session_id: self.session_id,
            player_id: self.player.id,
            display_name: self.player.display_name.clone(),
            net: self.net.clone(),
        }
    }

    pub fn create_set_session(&self) -> Packet {
        Packet::notify(
            Components::UserSessions(UserSessions::SetSession),
            SetPlayer { player: self },
        )
    }

    pub fn encode(&self, slot: usize, writer: &mut TdfWriter) {
        writer.tag_empty_blob(b"BLOB");
        writer.tag_u8(b"EXID", 0);
        writer.tag_u32(b"GID", self.game_id);
        writer.tag_u32(b"LOC", 0x64654445);
        writer.tag_str(b"NAME", &self.player.display_name);
        writer.tag_u32(b"PID", self.player.id);
        self.net.tag_groups(b"PNET", writer);
        writer.tag_usize(b"SID", slot);
        writer.tag_u8(b"SLOT", 0);
        writer.tag_value(b"STAT", &self.state);
        writer.tag_u16(b"TIDX", 0xffff);
        writer.tag_u8(b"TIME", 0); /* Unix timestamp in millseconds */
        writer.tag_triple(b"UGID", (0, 0, 0));
        writer.tag_u32(b"UID", self.session_id);
        writer.tag_group_end();
    }

    pub fn encode_data(&self, writer: &mut TdfWriter) {
        self.net.tag_groups(b"ADDR", writer);
        writer.tag_str(b"BPS", "ea-sjc");
        writer.tag_str_empty(b"CTY");
        writer.tag_var_int_list_empty(b"CVAR");
        {
            writer.tag_map_start(b"DMAP", TdfType::VarInt, TdfType::VarInt, 1);
            writer.write_u32(0x70001);
            writer.write_u16(0x409a);
        }
        writer.tag_u16(b"HWFG", self.net.hardware_flags);
        {
            writer.tag_list_start(b"PSLM", TdfType::VarInt, 1);
            writer.write_u32(0xfff0fff);
        }
        writer.tag_value(b"QDAT", &self.net.qos);
        writer.tag_u8(b"UATT", 0);
        writer.tag_list_start(b"ULST", TdfType::Triple, 1);
        (4, 1, self.game_id).encode(writer);
        writer.tag_group_end();
    }
}

impl Drop for GamePlayer {
    fn drop(&mut self) {
        // Clear player game when game player is dropped
        let _ = self.addr.link.do_send(SetGameMessage { game: None });
    }
}

pub struct SetPlayer<'a> {
    pub player: &'a GamePlayer,
}

impl Encodable for SetPlayer<'_> {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_group(b"DATA");
        self.player.encode_data(writer);
        writer.tag_u32(b"USID", self.player.player.id);
    }
}
