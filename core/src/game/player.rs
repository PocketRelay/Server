use blaze_pk::{codec::Codec, packet::Packet, tag::ValueType, tagging::*};
use serde::Serialize;
use tokio::{join, sync::mpsc};
use utils::types::{GameID, PlayerID, SessionID};

use crate::blaze::{
    codec::{NetData, UpdateExtDataAttr},
    components::{Components, UserSessions},
};

use super::codec::PlayerState;

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

#[derive(Serialize)]
pub struct GamePlayerSnapshot {
    pub session_id: SessionID,
    pub player_id: PlayerID,
    pub display_name: String,
    pub net: NetData,
}

impl GamePlayer {
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

    pub fn encode(&self, slot: usize, output: &mut Vec<u8>) {
        tag_empty_blob(output, "BLOB");
        tag_u8(output, "EXID", 0);
        tag_u32(output, "GID", self.game_id);
        tag_u32(output, "LOC", 0x64654445);
        tag_str(output, "NAME", &self.display_name);
        tag_u32(output, "PID", self.player_id);
        self.net.tag_groups("PNET", output);
        tag_usize(output, "SID", slot);
        tag_u8(output, "SLOT", 0);
        tag_value(output, "STAT", &self.state);
        tag_u16(output, "TIDX", 0xffff);
        tag_u8(output, "TIME", 0);
        tag_triple(output, "UGID", &(0, 0, 0));
        tag_u32(output, "UID", self.session_id);
        tag_group_end(output);
    }

    pub fn encode_data(&self, output: &mut Vec<u8>) {
        self.net.tag_groups("ADDR", output);
        tag_str(output, "BPS", "ea-sjc");
        tag_empty_str(output, "CTY");
        tag_var_int_list_empty(output, "CVAR");
        {
            tag_map_start(output, "DMAP", ValueType::VarInt, ValueType::VarInt, 1);
            0x70001.encode(output);
            0x409a.encode(output);
        }
        tag_u16(output, "HWFG", self.net.hardware_flags);
        {
            tag_list_start(output, "PSLM", ValueType::VarInt, 1);
            0xfff0fff.encode(output);
        }
        tag_value(output, "QDAT", &self.net.qos);
        tag_u8(output, "UATT", 0);
        tag_list_start(output, "ULST", ValueType::Triple, 1);
        (4, 1, self.game_id).encode(output);
        tag_group_end(output);
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

impl Codec for PlayerUpdate<'_> {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_group_start(output, "DATA");
        self.player.encode_data(output);

        tag_group_start(output, "USER");
        tag_u32(output, "AID", self.player.player_id);
        tag_u32(output, "ALOC", 0x64654445);
        tag_empty_blob(output, "EXBB");
        tag_u8(output, "EXID", 0);
        tag_u32(output, "ID", self.player.player_id);
        tag_str(output, "NAME", &self.player.display_name);
        tag_group_end(output);
    }
}

pub struct SetPlayer<'a> {
    pub player: &'a GamePlayer,
}

impl Codec for SetPlayer<'_> {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_group_start(output, "DATA");
        self.player.encode_data(output);
        tag_u32(output, "USID", self.player.player_id);
    }
}
