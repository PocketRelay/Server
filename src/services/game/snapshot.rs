use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::{
    session::{data::NetData, models::game_manager::GameState},
    utils::types::{GameID, PlayerID},
};

use super::{AttrMap, Game, GamePlayer};

/// Snapshot of the current game state and players
#[derive(Serialize)]
pub struct GameSnapshot {
    /// The ID of the game the snapshot is for
    pub id: GameID,
    /// The current game state
    pub state: GameState,
    /// The current game setting
    pub setting: u16,
    /// The game attributes
    pub attributes: AttrMap,
    /// Snapshots of the game players
    pub players: Option<Box<[GamePlayerSnapshot]>>,
    /// The total number of players in the game
    pub total_players: usize,
    /// When the game was created
    pub created_at: DateTime<Utc>,
}

impl GameSnapshot {
    pub fn new(game: &Game, include_net: bool, include_players: bool) -> Self {
        let total_players: usize = game.players.len();
        let players = if include_players {
            let players = game
                .players
                .iter()
                .map(|value| GamePlayerSnapshot::new(value, include_net))
                .collect();
            Some(players)
        } else {
            None
        };

        Self {
            id: game.id,
            state: game.state,
            setting: game.settings.bits(),
            attributes: game.attributes.clone(),
            players,
            total_players,
            created_at: game.created_at,
        }
    }
}

/// Structure for taking a snapshot of the players current
/// state.
#[derive(Serialize)]
pub struct GamePlayerSnapshot {
    /// The player ID of the snapshot
    pub player_id: PlayerID,
    /// The player name of the snapshot
    pub display_name: Box<str>,
    /// The player net data of the snapshot if collected
    pub net: Option<Arc<NetData>>,
}

impl GamePlayerSnapshot {
    pub fn new(player: &GamePlayer, include_net: bool) -> Self {
        Self {
            player_id: player.player.id,
            display_name: Box::from(player.player.display_name.as_ref()),
            net: if include_net { player.net() } else { None },
        }
    }
}
