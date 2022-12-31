use super::codec::PlayerState;
use crate::{
    blaze::{
        codec::{NetData, UpdateExtDataAttr},
        components::{Components, UserSessions},
    },
    servers::main::session::SessionAddr,
    utils::types::{GameID, PlayerID, SessionID},
};
use blaze_pk::{codec::Encodable, packet::Packet, tag::TdfType, writer::TdfWriter};
use database::Player;
use serde::Serialize;

pub struct GamePlayer {
    pub game_id: GameID,
    /// Session player
    pub player: Player,
    /// Session address
    pub addr: SessionAddr,
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
    pub fn new(player: Player, net: NetData, addr: SessionAddr) -> Self {
        Self {
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
            session_id: self.addr.id,
            player_id: self.player.id,
            display_name: self.player.display_name.clone(),
            net: self.net.clone(),
        }
    }

    pub fn write_updates(&self, other: &GamePlayer) {
        other.addr.push(Packet::notify(
            Components::UserSessions(UserSessions::SessionDetails),
            PlayerUpdate { player: self },
        ));
        other.addr.push(Packet::notify(
            Components::UserSessions(UserSessions::UpdateExtendedDataAttribute),
            UpdateExtDataAttr {
                flags: 0x3,
                player_id: self.player.id,
            },
        ));
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
        writer.tag_u8(b"TIME", 0);
        writer.tag_triple(b"UGID", (0, 0, 0));
        writer.tag_u32(b"UID", self.addr.id);
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
        self.addr.set_game(None)
    }
}

pub struct PlayerUpdate<'a> {
    pub player: &'a GamePlayer,
}

impl Encodable for PlayerUpdate<'_> {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_group(b"DATA");
        self.player.encode_data(writer);

        writer.tag_group(b"USER");
        writer.tag_u32(b"AID", self.player.player.id);
        writer.tag_u32(b"ALOC", 0x64654445);
        writer.tag_empty_blob(b"EXBB");
        writer.tag_u8(b"EXID", 0);
        writer.tag_u32(b"ID", self.player.player.id);
        writer.tag_str(b"NAME", &self.player.player.display_name);
        writer.tag_group_end();
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
