mod shared;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::RwLock;
use crate::blaze::{Session, SessionArc, SessionGame};
use crate::blaze::errors::{GameError, GameResult};

type Player = Arc<Session>;
type PlayersList = Vec<Player>;

pub struct GameManager {
    games: RwLock<HashMap<u32, Arc<Game>>>,
    next_id: AtomicU32,
}

impl GameManager {
    pub fn new() -> Self {
        Self {
            games: RwLock::new(HashMap::new()),
            next_id: AtomicU32::new(1),
        }
    }

    pub async fn new_game(&self, attributes: HashMap<String, String>) -> Arc<Game> {
        let mut games = self.games.write().await;
        let id = self.next_id.fetch_add(1, Ordering::AcqRel);
        let game = Arc::new(Game::new(id, attributes));
        games.insert(id, game.clone());
        game
    }
}

pub struct Game {
    pub id: u32,
    state: u16,
    setting: u16,
    attributes: RwLock<HashMap<String, String>>,
    players: RwLock<PlayersList>,
}

impl Game {
    const GPVH: u64 = 0x5a4f2b378b715c6;
    const GSID: u64 = 0x4000000a76b645;
    const MAX_PLAYERS: usize = 4;

    pub fn new(id: u32, attributes: HashMap<String, String>) -> Self {
        Self {
            id,
            state: 0x1,
            setting: 0x11f,
            attributes: RwLock::new(attributes),
            players: RwLock::new(Vec::with_capacity(Self::MAX_PLAYERS)),
        }
    }

    /// Returns the current number of players present in the player list
    /// for this game.
    pub async fn player_count(&self) -> usize {
        let players = self.players.read().await;
        players.len()
    }

    pub async fn get_host(&self) -> GameResult<&Arc<Session>> {
        let players = self.players.read().await;
        let player = players.get(0)
            .ok_or(GameError::MissingHost)?;
        Ok(player)
    }

    pub async fn add_player(game: &Arc<Game>, session: &SessionArc) -> GameResult<()> {
        // Game is full cannot add anymore players
        if game.player_count() >= Self::MAX_PLAYERS {
            return Err(GameError::Full);
        }
        let session = session.clone();

        // Add the player to the players list returning the slot it was added to
        let slot = {
            let mut players = game.players.write().await;
            let slot = players.len();
            players.push(session);
            slot
        };

        // Set the player session game data
        {
            let mut session_data = session.data.write().await;
            session_data.game = Some(SessionGame {
                game: game.clone(),
                slot,
            })
        }

        Ok(())
    }
}