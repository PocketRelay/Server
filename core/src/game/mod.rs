pub mod enums;
pub mod rules;
mod shared;

use crate::blaze::components::{Components, GameManager, UserSessions};
use crate::blaze::errors::BlazeResult;
use crate::blaze::session::{Session, SessionArc};
use crate::blaze::shared::{NotifyAdminListChange, NotifyJoinComplete, SessionStateChange};
use crate::game::shared::{
    notify_game_setup, FetchExtendedData, NotifyAttribsChange, NotifyPlayerJoining,
    NotifyPlayerRemoved, NotifySettingChange, NotifyStateChange,
};
use blaze_pk::{OpaquePacket, Packets, TdfMap};
use log::{debug, error, warn};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::join;
use tokio::sync::RwLock;

use self::rules::RuleSet;
use self::shared::{HostMigrateFinished, HostMigrateStart};

pub struct Games {
    games: RwLock<HashMap<u32, Game>>,
    match_queue: RwLock<VecDeque<(SessionArc, RuleSet)>>,
    next_id: AtomicU32,
}

impl Games {
    pub fn new() -> Self {
        Self {
            games: RwLock::new(HashMap::new()),
            match_queue: RwLock::new(VecDeque::new()),
            next_id: AtomicU32::new(1),
        }
    }

    pub async fn new_game(
        &self,
        name: String,
        attributes: TdfMap<String, String>,
        setting: u16,
    ) -> BlazeResult<u32> {
        let mut games = self.games.write().await;
        let id = self.next_id.fetch_add(1, Ordering::AcqRel);
        let game = Game::new(id, name, attributes, setting);
        games.insert(id, game);
        Ok(id)
    }

    pub async fn add_player(&self, game_id: u32, session: &SessionArc) -> BlazeResult<bool> {
        let games = self.games.read().await;
        let Some(game) = games.get(&game_id) else {
            return Ok(false);
        };
        Ok(game.add_player(session).await)
    }

    /// Async handler for when a new game is created in order to update
    /// the queue checking if any of the other players rule sets match the
    /// details of the game
    pub async fn on_game_created(&self, game_id: u32) {
        let games = self.games.read().await;
        let Some(game) = games.get(&game_id) else {
            return;
        };

        debug!("Matchmaking game created. Checking queue for players...");
        let mut removed_ids = Vec::new();
        {
            let queue = self.match_queue.read().await;
            for (session, rules) in queue.iter() {
                if rules.matches(game).await && game.is_joinable().await {
                    debug!("Found player from queue. Adding them to the game.");
                    if game.add_player(session).await {
                        removed_ids.push(session.id);
                    } else {
                        break;
                    }
                }
            }
        }

        if removed_ids.len() > 0 {
            let queue = &mut *self.match_queue.write().await;
            queue.retain(|value| !removed_ids.contains(&value.0.id))
        }
    }

    /// Attempts to find a game that matches the players provided rule set
    /// or adds them to the matchmaking queue if one could not be found.
    pub async fn get_or_queue(&self, session: &SessionArc, rules: RuleSet) -> bool {
        let games = self.games.read().await;
        for game in games.values() {
            if rules.matches(game).await {
                println!("Found matching game {}", &game.name);

                if game.add_player(session).await {
                    return true;
                }
            }
        }

        // Update the player matchmaking data.
        {
            let session_data = &mut *session.data.write().await;
            session_data.matchmaking = true;
        }

        debug!("Updated player matchmaking data");

        // Push the player to the end of the queue
        let queue = &mut *self.match_queue.write().await;
        queue.push_back((session.clone(), rules));
        debug!("Added player to back of queue");

        false
    }

    /// Removes a player from the queue if it exists
    pub async fn remove_queue(&self, session: &Session) {
        let queue = &mut *self.match_queue.write().await;
        queue.retain(|value| value.0.id != session.id);
    }

    pub async fn update_mesh_connection(&self, id: u32, session: &SessionArc, target: u32) -> bool {
        let games = self.games.read().await;
        let Some(game) = games.get(&id) else { return false; };
        game.update_mesh_connection(session, target).await;
        true
    }

    pub async fn set_game_state(&self, id: u32, state: u16) -> bool {
        let games = self.games.read().await;
        let Some(game) = games.get(&id) else { return false; };
        game.set_state(state).await;
        true
    }

    pub async fn set_game_setting(&self, id: u32, setting: u16) -> bool {
        let games = self.games.read().await;
        let Some(game) = games.get(&id) else { return false; };
        game.set_setting(setting).await;
        true
    }

    pub async fn set_game_attributes(&self, id: u32, attributes: TdfMap<String, String>) -> bool {
        let games = self.games.read().await;
        let Some(game) = games.get(&id) else { return false; };
        game.set_attributes(attributes).await;
        true
    }

    pub async fn remove_player(&self, id: u32, pid: u32) -> bool {
        let games = self.games.read().await;
        let Some(game) = games.get(&id) else { return false; };
        game.remove_by_id(pid).await;
        if game.is_empty().await {
            drop(game);
            drop(games);
            self.release(id).await;
        };
        true
    }

    pub async fn release(&self, game_id: u32) {
        let game = {
            let mut games = self.games.write().await;
            games.remove(&game_id)
        };

        let Some(game) = game else {return;};

        let players = &mut *game.players.write().await;
        let futures: Vec<_> = players.iter().map(|value| value.clear_game()).collect();
        let _ = futures::future::join_all(futures).await;
        players.clear();
    }

    pub async fn release_player(&self, player: &Session) {
        // Removes a player from the queue if it exists
        {
            let queue = &mut *self.match_queue.write().await;
            queue.retain(|value| value.0.id != player.id);
        }

        debug!("Releasing player (Session ID: {})", player.id);

        let game_id = {
            let session_data = &mut *player.data.write().await;
            let Some(game_id) = session_data.game.take() else { return; };
            game_id
        };

        let games = self.games.read().await;
        let Some(game) = games.get(&game_id) else { 
            debug!(
                "Game session was referencing didn't exist (GID: {}, SID: {})", 
                game_id, player.id
            );
            return;
        };

        debug!(
            "Releasing player from game (Name: {}, ID: {}, Session ID: {})",
            &game.name, game_id, player.id
        );

        game.remove_player(player).await;
        debug!("Checking if game can be removed");

        if game.is_empty().await {
            drop(game);
            drop(games);
            self.release(game_id).await;
        };
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

    pub async fn is_empty(&self) -> bool {
        self.player_count().await == 0
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

    pub async fn set_player_state(&self, session: &SessionArc, state: u8) {
        let player_id = {
            let session_data = &mut *session.data.write().await;
            session_data.state = state;
            session_data.id_safe()
        };

        let packet = Packets::notify(
            Components::GameManager(GameManager::GamePlayerStateChange),
            &SessionStateChange {
                gid: self.id,
                pid: player_id,
                state,
            },
        );
        self.push_all(&packet).await;
    }

    pub async fn update_mesh_connection(&self, session: &SessionArc, target: u32) {
        if !self.is_player(session).await {
            self.set_player_state(session, 2).await;
            return;
        }

        self.set_player_state(session, 4).await;

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
        session.clear_game().await;
        let Some(slot) = self.remove_session(session).await else {
            debug!("Player wasn't apart of that game");
            return;
        };
        self.notify_player_removed(session).await;
        self.notify_admin_removed(session).await;
        self.notify_fetch_data(session).await;
        debug!("Done removing player");
        if slot == 0 {
            self.migrate_host(session).await;
        }
    }

    /// Removes the provided session from the players list of this game
    /// and clears the game state stored on the session. Returning the slot
    /// that the player was in if it existed.
    ///
    /// `session` The session to remove.
    async fn remove_session(&self, session: &Session) -> Option<usize> {
        let mut players = self.players.write().await;
        let index = players.iter().position(|value| value.id == session.id)?;
        players.remove(index);
        debug!("Removed session from players list (SID: {})", session.id);
        Some(index)
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

        let packet = old_host.create_client_update().await;
        join!(self.push_all(&packet), old_host.write(&packet));

        debug!("Finished host migration");
    }

    /// Notifies all the players in the game that host migration has
    /// started and that the new host is the provided.
    ///
    /// `new_host` The newly decided host for the game.
    async fn notify_migration_start(&self, new_host: &SessionArc) {
        let host_id = {
            let host_data = new_host.data.read().await;
            let host_id = host_data.id_safe();
            host_id
        };

        let packet = Packets::notify(
            Components::GameManager(GameManager::HostMigrationStart),
            &HostMigrateStart {
                id: self.id,
                host: host_id,
                pmig: 0x2,
                // Should always be using the player which was in the second slot
                slot: 0x1,
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

    pub async fn add_player(&self, session: &SessionArc) -> bool {
        // Add the player to the players list returning the slot it was added to
        let slot = {
            let mut players = self.players.write().await;
            let player_count = players.len();

            // Game is full cannot add anymore players
            if player_count >= Self::MAX_PLAYERS {
                error!(
                    "Tried to add player to full game (SID: {}, GID: {})",
                    session.id, self.id,
                );
                return false;
            }

            players.push(session.clone());
            player_count
        };

        // Set the player session game data
        session.set_game(self.id).await;

        let is_host = slot == 0;

        // Don't send if this is the host joining
        if !is_host {
            // Update session details for other players and send join notifies
            debug!("Creating join notify");
            let packet = {
                let session_data = session.data.read().await;
                Packets::notify(
                    Components::GameManager(GameManager::PlayerJoining),
                    &NotifyPlayerJoining {
                        id: self.id,
                        slot,
                        session: &session_data,
                    },
                )
            };
            debug!("Pushing join notify to players");
            self.push_all(&packet).await;
        }

        debug!("Updating clients");
        self.update_clients_for(session).await;

        let setup = notify_game_setup(self, is_host, &session).await;
        debug!("Finished generating notify packet");

        session.write(&setup).await;
        debug!("Finished writing notify packet");

        let packet = session.create_client_update().await;
        self.push_all(&packet).await;

        debug!("Finished adding player");

        true
    }
}
