mod shared;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use blaze_pk::{OpaquePacket, Packets, TdfMap};
use tokio::sync::RwLock;
use crate::blaze::{Session, SessionArc, SessionGame};
use crate::blaze::components::{Components, GameManager};
use crate::blaze::errors::{BlazeError, BlazeResult, GameError, GameResult};
use crate::game::shared::{notify_game_setup, NotifyPlayerJoining};

pub struct Games {
    games: RwLock<HashMap<u32, Arc<Game>>>,
    next_id: AtomicU32,
}

impl Games {
    pub fn new() -> Self {
        Self {
            games: RwLock::new(HashMap::new()),
            next_id: AtomicU32::new(1),
        }
    }

    pub async fn new_game(
        &self,
        name: String,
        attributes: TdfMap<String, String>,
        setting: u16,
    ) -> Arc<Game> {
        let mut games = self.games.write().await;
        let id = self.next_id.fetch_add(1, Ordering::AcqRel);
        let game = Arc::new(Game::new(id, name, attributes, setting));
        games.insert(id, game.clone());
        game
    }
}

pub struct Game {
    pub id: u32,
    name: String,
    state: u16,
    setting: u16,
    attributes: RwLock<TdfMap<String, String>>,
    players: RwLock<Vec<SessionArc>>,
}

impl Game {
    const GPVH: u64 = 0x5a4f2b378b715c6;
    const GSID: u64 = 0x4000000a76b645;
    const MAX_PLAYERS: usize = 4;

    pub fn new(
        id: u32,
        name: String,
        attributes: TdfMap<String, String>,
        setting: u16,
    ) -> Self {
        Self {
            id,
            name,
            state: 0x1,
            setting,
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

    pub async fn get_host(&self) -> GameResult<SessionArc> {
        let players = self.players.read().await;
        let player = players.get(0)
            .ok_or(GameError::MissingHost)?;
        Ok(player.clone())
    }

    pub async fn push_all(&self, packet: &OpaquePacket) -> GameResult<()> {
        let players = &*self.players.read().await;
        for player in players {
            player.write_packet(packet).await?;
            // TODO: Handle disconnects here.
        }
        Ok(())
    }

    pub async fn add_player(game: &Arc<Game>, session: &SessionArc) -> BlazeResult<()> {
        // Game is full cannot add anymore players
        if game.player_count().await >= Self::MAX_PLAYERS {
            return Err(BlazeError::Game(GameError::Full));
        }

        // Add the player to the players list returning the slot it was added to
        let slot = {
            let mut players = game.players.write().await;
            let slot = players.len();
            players.push(session.clone());
            slot
        };

        // Set the player session game data
        {
            let mut session_data = session
                .data
                .write().await;
            session_data.game = Some(SessionGame {
                game: game.clone(),
                slot,
            })
        }

        // Joining player is not the host player
        if slot != 0 {
            let join_notify = {
                let session_data = session.data.read().await;
                let content = NotifyPlayerJoining {
                    id: 0,
                    session: &session_data,
                };
                Packets::notify(
                    Components::GameManager(GameManager::PlayerJoining),
                    &content,
                )
            };

            // Update session details for other players and send join notifies
            {
                let players = &*game.players.read().await;
                for player in players {
                    player.write_packet(&join_notify).await?;
                    // TODO: Handle disconnects here.
                    if player.id != session.id {
                        player.update_for(&session).await?;
                    }
                }
            }
        }

        let setup = notify_game_setup(game, &session).await?;
        session.write_packet(&setup).await?;
        session.update_client().await?;

        Ok(())
    }
}