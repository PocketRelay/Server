pub mod enums;
pub mod matchmaking;
mod shared;

use crate::blaze::components::{Components, GameManager, UserSessions};
use crate::blaze::errors::{BlazeError, BlazeResult, GameError, GameResult};
use crate::blaze::shared::{NotifyAdminListChange, NotifyJoinComplete};
use crate::blaze::{Session, SessionArc};
use crate::game::shared::{
    notify_game_setup, FetchExtendedData, NotifyAttribsChange, NotifyPlayerJoining,
    NotifyPlayerRemoved, NotifySettingChange, NotifyStateChange,
};
use blaze_pk::{OpaquePacket, Packets, TdfMap};
use log::{debug, warn};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::join;
use tokio::sync::RwLock;

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

    pub async fn release(&self, game: GameArc) {
        {
            let mut games = self.games.write().await;
            games.remove(&game.id);
        }

        let players = &mut *game.players.write().await;
        let futures: Vec<_> = players.iter().map(|value| value.clear_game()).collect();
        let _ = futures::future::join_all(futures).await;
        players.clear();
    }

    pub async fn release_player(&self, player: &Session) {
        debug!("Releasing player (Session ID: {})", player.id);

        let game = {
            let session_data = &mut *player.data.write().await;
            let Some(game) = session_data.game.take() else { return; };
            game.game
        };

        debug!(
            "Releasing player from game (Name: {}, ID: {}, Session ID: {})",
            &game.name, &game.id, player.id
        );
        game.remove_player(player).await;
        debug!("Checking if game can be removed");
        self.remove_if_empty(game).await;
    }

    pub async fn remove_if_empty(&self, game: GameArc) {
        if game.player_count().await > 0 {
            return;
        }
        debug!("Removing empty game (Name: {}, ID: {}", &game.name, game.id);
        self.release(game).await;
    }
}

pub type GameArc = Arc<Game>;

pub struct Game {
    pub id: u32,
    pub name: String,
    pub data: RwLock<GameData>,
    pub players: RwLock<Vec<SessionArc>>,
}

impl Drop for Game {
    fn drop(&mut self) {
        debug!("Game {} {} has been dropped", self.name, self.id)
    }
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

    pub async fn push_all(&self, packet: &OpaquePacket) {
        let players = &*self.players.read().await;
        let futures: Vec<_> = players
            .iter()
            .map(|value| value.write_packet(packet))
            .collect();

        // TODO: Handle errors for each players
        let _ = futures::future::join_all(futures).await;
    }

    pub async fn push_all_excl_host(&self, packet: &OpaquePacket) {
        let players = &*self.players.read().await;
        let futures: Vec<_> = players
            .iter()
            .skip(1)
            .map(|value| value.write_packet(packet))
            .collect();

        // TODO: Handle errors for each players
        let _ = futures::future::join_all(futures).await;
    }

    pub async fn push_all_list(&self, packets: &Vec<OpaquePacket>) {
        let players = &*self.players.read().await;
        let futures: Vec<_> = players
            .iter()
            .map(|value| value.write_packets(packets))
            .collect();
        // TODO: Handle errors for each players
        let _ = futures::future::join_all(futures).await;
    }

    pub async fn set_state(&self, state: u16) {
        {
            let data = &mut *self.data.write().await;
            data.state = state;
        }

        let packet = Packets::notify(
            Components::GameManager(GameManager::GameStateChange),
            &NotifyStateChange { id: self.id, state },
        );
        self.push_all(&packet).await;
    }

    pub async fn set_setting(&self, setting: u16) {
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
        self.push_all(&packet).await;
    }

    pub async fn set_attributes(&self, attributes: TdfMap<String, String>) {
        {
            let data = &mut *self.data.write().await;
            data.attributes.extend(attributes)
        }

        let packet = {
            let data = self.data.read().await;
            Packets::notify(
                Components::GameManager(GameManager::GameAttribChange),
                &NotifyAttribsChange {
                    id: self.id,
                    attributes: &data.attributes,
                },
            )
        };
        self.push_all(&packet).await;
    }

    pub async fn update_mesh_connection(&self, session: &SessionArc) {
        if !self.is_player(session).await {
            session.set_state(2).await;
            return;
        }

        session.set_state(4).await;

        debug!("Updating Mesh Connection");

        let host_id = {
            let players = self.players.read().await;
            let Some(host) = players.get(0) else {
                debug!("Game didn't have host unable to connect mesh");
                return;
            };
            let session_data = host.data.read().await;
            session_data.player_id_safe()
        };

        debug!("Mesh host ID: {}", host_id);

        let pid = {
            let session_data = session.data.read().await;
            session_data.player_id_safe()
        };

        debug!("Mesh player ID: {}", pid);

        let packet_a = Packets::notify(
            Components::GameManager(GameManager::PlayerJoinCompleted),
            &NotifyJoinComplete { gid: self.id, pid },
        );

        let packet_b = Packets::notify(
            Components::GameManager(GameManager::AdminListChange),
            &NotifyAdminListChange {
                alst: pid,
                gid: self.id,
                oper: 0,
                uid: host_id,
            },
        );

        let packets = vec![packet_a, packet_b];
        self.push_all_list(&packets).await;

        debug!("Finished updating mesh connections");
    }

    pub async fn find_player_by_id(&self, id: u32) -> Option<SessionArc> {
        let players = &*self.players.read().await;
        for player in players {
            let Some(player_id) = player.player_id().await else { continue; };
            if player_id == id {
                return Some(player.clone());
            }
        }
        None
    }

    async fn is_player(&self, session: &Session) -> bool {
        let players = self.players.read().await;
        players.iter().any(|value| value.id == session.id)
    }

    pub async fn remove_by_id(&self, id: u32) {
        debug!(
            "Attempting to remove player from game (PID: {}, GID: {})",
            id, self.id
        );
        if let Some(player) = self.find_player_by_id(id).await {
            debug!("Found player to remove. Removing player");
            self.remove_player(&player).await;
        } else {
            warn!(
                "Unable to find player with (ID: {}) in game (Name: {}, ID: {})",
                id, self.name, self.id
            );
        }
    }

    pub async fn remove_player(&self, session: &Session) {
        {
            let mut players = self.players.write().await;
            players.retain(|value| value.id != session.id);
            debug!("Removed player from players list (ID: {})", session.id)
        }

        let player_id = {
            let session_data = &mut *session.data.write().await;
            session_data.game = None;
            if let Some(player) = &session_data.player {
                debug!(
                    "Removing player {} from game {}",
                    player.display_name, self.id
                );
                self.id
            } else {
                debug!("Removing session {} from game {}", session.id, self.id);
                1
            }
        };

        let packet = Packets::notify(
            Components::GameManager(GameManager::PlayerRemoved),
            &NotifyPlayerRemoved {
                id: self.id,
                pid: player_id,
            },
        );

        join!(self.push_all(&packet), session.write_packet(&packet));

        debug!("Sent removal notify");

        let players = self.players.read().await;
        let Some(host) = players.get(0) else {
            debug!("Migrating host");
            self.migrate_host().await;
            return;
        };

        let host_id = host.player_id_safe().await;

        let packet = Packets::notify(
            Components::GameManager(GameManager::AdminListChange),
            &NotifyAdminListChange {
                alst: session.player_id_safe().await,
                gid: self.id,
                oper: 1,
                uid: host_id,
            },
        );

        self.push_all(&packet).await;
        debug!("Sent admin list changed notify");

        {
            let host_packet = Packets::notify(
                Components::UserSessions(UserSessions::FetchExtendedData),
                &FetchExtendedData { id: host_id },
            );
            let packets = {
                let mut packets = Vec::with_capacity(players.len());
                for player in players {
                    let id = player.player_id_safe().await;
                    packets.push(Packets::notify(
                        Components::UserSessions(UserSessions::FetchExtendedData),
                        &FetchExtendedData { id },
                    ));
                }
                packets
            };

            join!(
                self.push_all_excl_host(&host_packet),
                host.write_packets(&packets)
            );
        };
    }

    pub async fn migrate_host(&self) {
        // TODO: Implement
    }

    pub async fn is_joinable(&self) -> bool {
        self.player_count().await < Self::MAX_PLAYERS
    }

    pub async fn update_clients_for(&self, session: &SessionArc) {
        debug!("Updating session information of other players");
        let players = &*self.players.read().await;

        let futures: Vec<_> = players
            .iter()
            .filter(|value| value.id != session.id)
            .map(|value| value.update_for(session))
            .collect();

        let _ = futures::future::join_all(futures).await;

        debug!("Done updating session information");
    }

    pub async fn add_player(game: &GameArc, session: &SessionArc) -> BlazeResult<()> {
        // Add the player to the players list returning the slot it was added to
        let slot = {
            let mut players = game.players.write().await;
            let player_count = players.len();

            // Game is full cannot add anymore players
            if player_count >= Self::MAX_PLAYERS {
                return Err(BlazeError::Game(GameError::Full));
            }

            players.push(session.clone());
            player_count
        };

        // Set the player session game data
        session.set_game(game.clone(), slot).await;

        // Don't send if this is the host joining
        if slot != 0 {
            // Update session details for other players and send join notifies
            debug!("Creating join notify");
            let packet = {
                let session_data = session.data.read().await;
                Packets::notify(
                    Components::GameManager(GameManager::PlayerJoining),
                    &NotifyPlayerJoining {
                        id: game.id,
                        session: &session_data,
                    },
                )
            };
            debug!("Pushing join notify to players");
            game.push_all(&packet).await;
        }

        debug!("Updating clients");
        game.update_clients_for(session).await;

        let setup = notify_game_setup(game, &session).await?;
        debug!("Finished generating notify packet");

        session.write_packet(&setup).await;
        debug!("Finished writing notify packet");

        session.update_client().await;

        debug!("Finished adding player");

        Ok(())
    }
}
