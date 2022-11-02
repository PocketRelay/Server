pub mod enums;
pub mod matchmaking;
mod shared;

use crate::blaze::components::{Components, GameManager};
use crate::blaze::errors::{BlazeError, BlazeResult, GameError, GameResult};
use crate::blaze::shared::{NotifyAdminListChange, NotifyJoinComplete};
use crate::blaze::{SessionArc, SessionGame};
use crate::game::shared::{
    notify_game_setup, NotifyAttribsChange, NotifyPlayerJoining, NotifyPlayerRemoved,
    NotifySettingChange, NotifyStateChange,
};
use blaze_pk::{OpaquePacket, Packets, TdfMap};
use log::debug;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::try_join;

pub struct Games {
    games: RwLock<HashMap<u32, GameArc>>,
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

    pub async fn find_by_id(&self, id: u32) -> Option<Arc<Game>> {
        let games = self.games.read().await;
        games.get(&id).cloned()
    }
}

pub type GameArc = Arc<Game>;

pub struct Game {
    pub id: u32,
    pub name: String,
    pub data: RwLock<GameData>,
    pub players: RwLock<Vec<SessionArc>>,
}

pub struct GameData {
    pub state: u16,
    pub setting: u16,
    pub attributes: TdfMap<String, String>,
}

impl Game {
    const GPVH: u64 = 0x5a4f2b378b715c6;
    const GSID: u64 = 0x4000000a76b645;
    const MAX_PLAYERS: usize = 4;

    pub fn new(id: u32, name: String, attributes: TdfMap<String, String>, setting: u16) -> Self {
        Self {
            id,
            name,
            data: RwLock::new(GameData {
                state: 0x1,
                setting,
                attributes,
            }),
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
        let player = players.get(0).ok_or(GameError::MissingHost)?;
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

    pub async fn set_state(&self, state: u16) -> BlazeResult<()> {
        {
            let mut data = self.data.write().await;
            (*data).state = state;
        }

        let packet = Packets::notify(
            Components::GameManager(GameManager::GameStateChange),
            &NotifyStateChange { id: self.id, state },
        );
        self.push_all(&packet).await?;
        Ok(())
    }

    pub async fn set_setting(&self, setting: u16) -> BlazeResult<()> {
        {
            let mut data = self.data.write().await;
            (*data).setting = setting;
        }

        let packet = Packets::notify(
            Components::GameManager(GameManager::GameSettingsChange),
            &NotifySettingChange {
                id: self.id,
                setting,
            },
        );
        self.push_all(&packet).await?;
        Ok(())
    }

    pub async fn set_attributes(&self, attributes: TdfMap<String, String>) -> BlazeResult<()> {
        {
            let mut data = self.data.write().await;
            (*data).attributes.extend(attributes)
        }

        let packet = {
            let data = self.data.read().await;
            Packets::notify(
                Components::GameManager(GameManager::GameSettingsChange),
                &NotifyAttribsChange {
                    id: self.id,
                    attributes: &data.attributes,
                },
            )
        };
        self.push_all(&packet).await?;
        Ok(())
    }

    pub async fn update_mesh_connection(&self, session: &SessionArc) -> BlazeResult<()> {
        session.set_state(4).await?;

        let pid = {
            let session_data = session.data.read().await;
            session_data.player_id_safe()
        };

        let packet_a = Packets::notify(
            Components::GameManager(GameManager::PlayerJoinCompleted),
            &NotifyJoinComplete { gid: self.id, pid },
        );

        let packet_b = Packets::notify(
            Components::GameManager(GameManager::PlayerJoinCompleted),
            &NotifyAdminListChange {
                alst: pid,
                gid: self.id,
                oper: 0,
                uid: pid,
            },
        );

        // May need to refactor possible issues could arise.

        try_join!(self.push_all(&packet_a), self.push_all(&packet_b))?;

        Ok(())
    }

    pub async fn remove_by_id(&self, id: u32) -> BlazeResult<()> {
        let players = self.players.read().await;
        let player = players.iter().find(|player| player.id == id);
        if let Some(player) = player {
            self.remove_player(player).await?;
        }
        Ok(())
    }

    pub async fn remove_player(&self, session: &SessionArc) -> BlazeResult<()> {
        let session_data = session.data.read().await;

        if let Some(player) = &session_data.player {
            debug!(
                "Removing player {} from game {}",
                player.display_name, self.id
            )
        } else {
            debug!("Removing session {} from game {}", session.id, self.id);
        }

        {
            let packet = Packets::notify(
                Components::GameManager(GameManager::PlayerRemoved),
                &NotifyPlayerRemoved {
                    id: self.id,
                    pid: session_data.player_id_safe(),
                },
            );
            self.push_all(&packet).await?;
        }

        drop(session_data);

        {
            let mut players = self.players.write().await;
            players.retain(|value| value.id != session.id);
        }

        // TODO: Host migration notify adminlistchange

        Ok(())
    }

    pub async fn is_joinable(&self) -> bool {
        self.player_count().await < Self::MAX_PLAYERS
    }

    pub async fn add_player(game: &GameArc, session: &SessionArc) -> BlazeResult<()> {
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
            let mut session_data = session.data.write().await;
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
