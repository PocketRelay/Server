use super::models::MeshState;
use crate::{
    session::{Session, SetGameMessage},
    utils::{
        models::NetData,
        types::{GameID, PlayerID, SessionID},
    },
};
use blaze_pk::writer::TdfWriter;
use database::Player;
use interlink::prelude::Link;
use serde::Serialize;

/// Player structure containing details and state for a player
/// within a game
pub struct GamePlayer {
    /// ID of the session associated to this player
    pub session_id: SessionID,
    /// Session player
    pub player: Player,
    /// Session address
    pub link: Link<Session>,
    /// Networking information for the player
    pub net: NetData,
    /// The mesh state of the player
    pub state: MeshState,
}

/// Structure for taking a snapshot of the players current
/// state.

#[derive(Serialize)]
pub struct GamePlayerSnapshot {
    /// The session ID of the snapshot
    pub session_id: SessionID,
    /// The player ID of the snapshot
    pub player_id: PlayerID,
    /// The player name of the snapshot
    pub display_name: String,
    /// The player net data of the snapshot if collected
    pub net: Option<NetData>,
}

impl GamePlayer {
    /// Creates a new game player structure with the provided player
    /// details
    ///
    /// `player` The session player
    /// `net`    The player networking details
    /// `addr`   The session address
    pub fn new(session_id: SessionID, player: Player, net: NetData, link: Link<Session>) -> Self {
        Self {
            session_id,
            player,
            link,
            net,
            state: MeshState::Connecting,
        }
    }

    pub fn set_game(&self, game: Option<GameID>) {
        let _ = self.link.do_send(SetGameMessage { game });
    }

    /// Takes a snapshot of the current player state
    /// for serialization
    pub fn snapshot(&self, include_net: bool) -> GamePlayerSnapshot {
        GamePlayerSnapshot {
            session_id: self.session_id,
            player_id: self.player.id,
            display_name: self.player.display_name.clone(),
            net: if include_net {
                Some(self.net.clone())
            } else {
                None
            },
        }
    }

    pub fn encode(&self, game_id: GameID, slot: usize, writer: &mut TdfWriter) {
        writer.tag_empty_blob(b"BLOB");
        writer.tag_u8(b"EXID", 0);
        writer.tag_u32(b"GID", game_id);
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
}

impl Drop for GamePlayer {
    fn drop(&mut self) {
        // Clear player game when game player is dropped
        self.set_game(None);
    }
}
