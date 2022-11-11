pub mod enums;
pub mod matchmaking;
mod shared;

use crate::blaze::components::{Components, GameManager, UserSessions};
use crate::blaze::errors::BlazeResult;
use crate::blaze::session::{Session, SessionArc};
use crate::blaze::shared::{NotifyAdminListChange, NotifyJoinComplete};
use crate::game::shared::{
    notify_game_setup, FetchExtendedData, NotifyAttribsChange, NotifyPlayerJoining,
    NotifyPlayerRemoved, NotifySettingChange, NotifyStateChange,
};
use blaze_pk::{OpaquePacket, Packets, TdfMap};
use log::{debug, error, warn};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::join;
use tokio::sync::RwLock;

use self::shared::{HostMigrateFinished, HostMigrateStart};

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
            debug!("Game not empy. Leaving it.");
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
        let futures: Vec<_> = players.iter().map(|value| value.write(packet)).collect();

        // TODO: Handle errors for each players
        let _ = futures::future::join_all(futures).await;
    }

    pub async fn push_all_excl_host(&self, packet: &OpaquePacket) {
        let players = &*self.players.read().await;
        let futures: Vec<_> = players
            .iter()
            .skip(1)
            .map(|value| value.write(packet))
            .collect();

        // TODO: Handle errors for each players
        let _ = futures::future::join_all(futures).await;
    }

    pub async fn push_all_list(&self, packets: &Vec<OpaquePacket>) {
        let players = &*self.players.read().await;
        let futures: Vec<_> = players
            .iter()
            .map(|value| value.write_all(packets))
            .collect();
        // TODO: Handle errors for each players
        let _ = futures::future::join_all(futures).await;
    }

    pub async fn flush_players(&self) {
        let players = &*self.players.read().await;
        let futures: Vec<_> = players.iter().map(|value| value.flush()).collect();
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

    pub async fn update_mesh_connection(&self, session: &SessionArc, target: u32) {
        if !self.is_player(session).await {
            session.set_state(2).await;
            return;
        }

        session.set_state(4).await;

        debug!("Updating Mesh Connection");

        let pid = {
            let session_data = session.data.read().await;
            session_data.id_safe()
        };

        if let None = self.find_player_by_id(target).await {
            return;
        }

        debug!("Mesh player ID: {}", pid);

        let host_id = {
            let players = self.players.read().await;
            let Some(host) = players.get(0) else {
                debug!("Game didn't have host unable to connect mesh");
                return;
            };
            let session_data = host.data.read().await;
            session_data.id_safe()
        };

        debug!("Mesh host ID: {}", host_id);

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

    /// Handles removing a player from the game and updating all the
    /// other players that the player has been removed.
    ///
    /// `session` The removed player.
    pub async fn remove_player(&self, session: &Session) {
        let game_slot = session.clear_game().await;
        self.remove_session(session).await;
        self.notify_player_removed(session).await;
        self.notify_admin_removed(session).await;
        self.notify_fetch_data(session).await;
        debug!("Done removing player");

        let Some(game_slot) = game_slot else {
            debug!("Player was missing game slot");
            return;
        };

        if game_slot == 0 {
            self.migrate_host(session).await;
        }
    }

    /// Removes the provided session from the players list of this game
    /// and clears the game state stored on the session
    ///
    /// `session` The session to remove.
    async fn remove_session(&self, session: &Session) {
        let mut players = self.players.write().await;
        players.retain(|value| value.id != session.id);
        debug!("Removed session from players list (SID: {})", session.id)
    }

    /// Notifies all players in the game and the provided session that
    /// the provided session was removed from the game.
    ///
    /// `session` The session that was removed from the game
    async fn notify_player_removed(&self, session: &Session) {
        let player_id = session.player_id_safe().await;
        debug!(
            "Removing session from game (SID: {}, PID: {}, GID: {})",
            session.id, player_id, self.id
        );
        let packet = Packets::notify(
            Components::GameManager(GameManager::PlayerRemoved),
            &NotifyPlayerRemoved {
                id: self.id,
                pid: player_id,
            },
        );
        join!(self.push_all(&packet), session.write(&packet));
        debug!("Notified clients of removed player");
    }

    /// Notifies all players in the game that the a player was
    /// removed from the admin list. Will migrate the host
    /// player if one is not present.
    ///
    /// `session` The player removed from the admin list
    /// `host_id` The ID of the host player for this game
    async fn notify_admin_removed(&self, session: &Session) {
        let host_id = {
            let players = &*self.players.read().await;
            if let Some(host) = players.first() {
                host.player_id_safe().await
            } else {
                // Game has become empty because all players are gone.
                return;
            }
        };

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
        debug!("Notified clients of admin list change");
    }

    /// Notifies all the players in the game to fetch the extended
    /// data for the provided session and the session to do the same
    /// for all the players.
    ///
    /// `session` The session to fetch data
    async fn notify_fetch_data(&self, session: &Session) {
        let session_id = session.player_id_safe().await;
        let session_packet = Packets::notify(
            Components::UserSessions(UserSessions::FetchExtendedData),
            &FetchExtendedData { id: session_id },
        );

        let player_ids = {
            let players = &*self.players.read().await;
            let player_ids = players
                .iter()
                .map(|value| value.player_id_safe())
                .collect::<Vec<_>>();
            futures::future::join_all(player_ids).await
        };

        let mut player_packets = Vec::with_capacity(player_ids.len());
        for player_id in player_ids {
            player_packets.push(Packets::notify(
                Components::UserSessions(UserSessions::FetchExtendedData),
                &FetchExtendedData { id: player_id },
            ));
        }

        join!(
            self.push_all(&session_packet),
            session.write_all(&player_packets)
        );
    }

    /// Unimplemented host migration functionality
    /// returning the player ID of the new host if
    /// one is available
    pub async fn migrate_host(&self, old_host: &Session) {
        let players = &*self.players.read().await;
        let Some(new_host) = players.first() else {
            // There is no other players available to become host.
            return;
        };
        debug!("Starting host migration");
        self.notify_migration_start(new_host).await;
        self.set_state(0x82).await;
        self.notify_migration_finished().await;
        self.update_player_slots(players, old_host).await;
        debug!("Finished host migration");
    }

    /// Notifies all the players in the game that host migration has
    /// started and that the new host is the provided.
    ///
    /// `new_host` The newly decided host for the game.
    async fn notify_migration_start(&self, new_host: &SessionArc) {
        let (old_slot, host_id) = {
            let host_data = new_host.data.read().await;
            let old_slot = host_data.game.as_ref().map(|value| value.slot).unwrap_or(0);
            let host_id = host_data.id_safe();
            (old_slot, host_id)
        };

        let packet = Packets::notify(
            Components::GameManager(GameManager::HostMigrationStart),
            &HostMigrateStart {
                id: self.id,
                host: host_id,
                pmig: 0x2,
                slot: old_slot,
            },
        );

        self.push_all(&packet).await;
    }

    /// Notifies all the players that host migration is complete
    async fn notify_migration_finished(&self) {
        let packet = Packets::notify(
            Components::GameManager(GameManager::HostMigrationFinished),
            &HostMigrateFinished { id: self.id },
        );
        self.push_all(&packet).await;
    }

    /// Updates all the player slots ensuring they in the same
    /// slot that matches the one stored on the session. Will
    /// send client updates to ensure the client is correct
    ///
    /// `players` The players list for the game
    async fn update_player_slots(&self, players: &Vec<SessionArc>, old_host: &Session) {
        debug!("Updating player slots");
        for (slot, player) in players.iter().enumerate() {
            let player_data = &mut *player.data.write().await;
            let Some(game) = &mut player_data.game else { continue; };
            game.slot = slot;
        }

        let packet = old_host.create_client_update().await;
        join!(self.push_all(&packet), old_host.write(&packet));
        debug!("Finished updating player")
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
                error!(
                    "Tried to add player to full game (SID: {}, GID: {})",
                    session.id, game.id,
                );
                return Ok(());
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

        session.write(&setup).await;
        debug!("Finished writing notify packet");

        let packet = session.create_client_update().await;
        game.push_all(&packet).await;

        debug!("Finished adding player");

        Ok(())
    }
}
