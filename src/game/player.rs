use super::codec::PlayerState;
use crate::{
    blaze::{
        codec::{NetData, UpdateExtDataAttr},
        components::{Components, UserSessions},
    },
    utils::types::{GameID, PlayerID, SessionID},
};
use blaze_pk::{codec::Encodable, packet::Packet, tag::TdfType, writer::TdfWriter};
use serde::Serialize;
use tokio::{join, sync::mpsc};

pub struct GamePlayer {
    pub game_id: GameID,
    /// The ID of the session for the player
    pub session_id: SessionID,
    /// The player ID
    pub player_id: PlayerID,
    /// The player display name
    pub display_name: String,
    /// Networking information for the player
    pub net: NetData,
    /// State of the game player
    pub state: PlayerState,
    /// Sender for sending messages to the session
    pub message_sender: mpsc::Sender<SessionMessage>,
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
    /// `session_id`     The session ID of the player
    /// `player_id`      The ID of the player
    /// `display_name`   The display name of the player
    /// `net`            The player networking details
    /// `message_sender` The message sender for sending session messages
    pub fn new(
        session_id: SessionID,
        player_id: PlayerID,
        display_name: String,
        net: NetData,
        message_sender: mpsc::Sender<SessionMessage>,
    ) -> Self {
        Self {
            session_id,
            player_id,
            display_name,
            net,
            game_id: 1,
            state: PlayerState::Connecting,
            message_sender,
        }
    }

    /// Takes a snapshot of the current player state
    /// for serialization
    pub fn snapshot(&self) -> GamePlayerSnapshot {
        GamePlayerSnapshot {
            session_id: self.session_id,
            player_id: self.player_id,
            display_name: self.display_name.clone(),
            net: self.net,
        }
    }

    pub async fn push(&self, packet: Packet) {
        self.message_sender
            .send(SessionMessage::Packet(packet))
            .await
            .ok();
    }

    pub async fn push_all(&self, packets: Vec<Packet>) {
        self.message_sender
            .send(SessionMessage::Packets(packets))
            .await
            .ok();
    }

    pub async fn exchange_update(&self, other: &GamePlayer) {
        join!(self.write_updates(other), other.write_updates(self));
    }

    pub async fn write_updates(&self, other: &GamePlayer) {
        let packets = vec![
            Packet::notify(
                Components::UserSessions(UserSessions::SessionDetails),
                PlayerUpdate { player: self },
            ),
            Packet::notify(
                Components::UserSessions(UserSessions::UpdateExtendedDataAttribute),
                UpdateExtDataAttr {
                    flags: 0x3,
                    player_id: self.player_id,
                },
            ),
        ];
        other.push_all(packets).await;
    }

    pub async fn set_game(&self, game: Option<GameID>) {
        self.message_sender
            .send(SessionMessage::SetGame(game))
            .await
            .ok();
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
        writer.tag_str(b"NAME", &self.display_name);
        writer.tag_u32(b"PID", self.player_id);
        self.net.tag_groups(b"PNET", writer);
        writer.tag_usize(b"SID", slot);
        writer.tag_u8(b"SLOT", 0);
        writer.tag_value(b"STAT", &self.state);
        writer.tag_u16(b"TIDX", 0xffff);
        writer.tag_u8(b"TIME", 0);
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

/// Describes messages that can be sent to a session
#[derive(Debug)]
pub enum SessionMessage {
    SetGame(Option<GameID>),
    Packet(Packet),
    Packets(Vec<Packet>),
}

pub struct PlayerUpdate<'a> {
    pub player: &'a GamePlayer,
}

impl Encodable for PlayerUpdate<'_> {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_group(b"DATA");
        self.player.encode_data(writer);

        writer.tag_group(b"USER");
        writer.tag_u32(b"AID", self.player.player_id);
        writer.tag_u32(b"ALOC", 0x64654445);
        writer.tag_empty_blob(b"EXBB");
        writer.tag_u8(b"EXID", 0);
        writer.tag_u32(b"ID", self.player.player_id);
        writer.tag_str(b"NAME", &self.player.display_name);
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
        writer.tag_u32(b"USID", self.player.player_id);
    }
}
