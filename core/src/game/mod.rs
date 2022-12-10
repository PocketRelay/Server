pub mod codec;
pub mod enums;
pub mod manager;
pub mod player;
pub mod rules;

use std::collections::HashMap;

use blaze_pk::{codec::Encodable, packet::Packet, types::TdfMap};

use log::{debug, warn};
use serde::Serialize;
use tokio::{join, sync::RwLock};
use utils::types::{GameID, GameSlot, PlayerID, SessionID};

use crate::blaze::components::{Components, GameManager, UserSessions};

use codec::{
    AdminListChange, AdminListOperation, AttributesChange, FetchExtendedData, GameDetails,
    GameDetailsType, GameState, HostMigrateFinished, HostMigrateStart, JoinComplete, PlayerJoining,
    PlayerRemoved, PlayerState, PlayerStateChange, RemoveReason, SettingChange, StateChange,
};
use player::{GamePlayer, GamePlayerSnapshot};

pub struct Game {
    /// Unique ID for this game
    pub id: GameID,
    /// Mutable data for this game
    pub data: RwLock<GameData>,
    /// The list of players in this game
    pub players: RwLock<Vec<GamePlayer>>,
    /// The number of the next available slot
    pub next_slot: RwLock<GameSlot>,
}

impl Drop for Game {
    fn drop(&mut self) {
        debug!("Game has been dropped (GID: {})", self.id)
    }
}

#[derive(Serialize)]
pub struct GameSnapshot {
    pub id: GameID,
    pub state: GameState,
    pub setting: u16,
    pub attributes: HashMap<String, String>,
    pub players: Vec<GamePlayerSnapshot>,
}

/// Attributes map type
pub type AttrMap = TdfMap<String, String>;

/// Structure for storing the mutable portion of
/// the game data
pub struct GameData {
    /// The current game state
    pub state: GameState,
    /// The current game setting
    pub setting: u16,
    /// The game attributes
    pub attributes: AttrMap,
}

impl GameData {
    fn new(setting: u16, attributes: AttrMap) -> Self {
        Self {
            state: GameState::Init,
            setting,
            attributes,
        }
    }
}

impl Game {
    /// Constant for the maximum number of players allowed in
    /// a game at one time. Used to determine a games full state
    const MAX_PLAYERS: usize = 4;

    /// Creates a new game with the provided details
    ///
    /// `id`         The unique game ID
    /// `attributes` The initial game attributes
    /// `setting`    The initial game setting
    pub fn new(id: GameID, attributes: AttrMap, setting: u16) -> Self {
        Self {
            id,
            data: RwLock::new(GameData::new(setting, attributes)),
            players: RwLock::new(Vec::new()),
            next_slot: RwLock::new(0),
        }
    }

    /// Takes a snapshot of the current game state for serialization
    pub async fn snapshot(&self) -> GameSnapshot {
        let data = &*self.data.read().await;
        let old_attributes = &data.attributes;
        let mut attributes = HashMap::with_capacity(old_attributes.len());
        for (key, value) in old_attributes.iter() {
            attributes.insert(key.to_owned(), value.to_owned());
        }

        let players = &*self.players.read().await;
        let players = players.iter().map(|value| value.snapshot()).collect();

        GameSnapshot {
            id: self.id,
            state: data.state,
            setting: data.setting,
            attributes,
            players,
        }
    }

    /// Writes the provided packet to all connected sessions.
    /// Does not wait for the write to complete just waits for
    /// it to be placed into each sessions write buffers.
    ///
    /// `packet` The packet to write
    async fn push_all(&self, packet: &Packet) {
        let players = &*self.players.read().await;
        let futures = players
            .iter()
            .map(|value| value.push(packet.clone()))
            .collect::<Vec<_>>();

        let _ = futures::future::join_all(futures).await;
    }

    /// Sends a notification packet to all the connected session
    /// with the provided component and contents
    ///
    /// `component` The packet component
    /// `contents`  The packet contents
    async fn notify_all<C: Encodable>(&self, component: Components, contents: C) {
        let packet = Packet::notify(component, contents);
        self.push_all(&packet).await;
    }

    /// Sets the current game state in the game data and
    /// sends an update notification to all connected clients
    /// notifying them of the changed state
    ///
    /// `state` The new state value
    pub async fn set_state(&self, state: GameState) {
        debug!("Updating game state (Value: {state:?})");
        {
            let data = &mut *self.data.write().await;
            data.state = state;
        }

        self.notify_all(
            Components::GameManager(GameManager::GameStateChange),
            StateChange { id: self.id, state },
        )
        .await;
    }

    /// Sets the current game setting in the game data and
    /// sends an update notification to all connected clients
    /// notifying them of the changed setting
    ///
    /// `setting` The new setting value
    pub async fn set_setting(&self, setting: u16) {
        debug!("Updating game setting (Value: {setting})");
        {
            let data = &mut *self.data.write().await;
            data.setting = setting;
        }

        self.notify_all(
            Components::GameManager(GameManager::GameSettingsChange),
            SettingChange {
                id: self.id,
                setting,
            },
        )
        .await;
    }

    /// Sets the current game attributes in the game data and
    /// sends an update notification to all connected clients
    /// notifying them of the changed attributes
    ///
    /// `attributes` The new attributes
    pub async fn set_attributes(&self, attributes: AttrMap) {
        debug!("Updating game attributes");
        let packet = Packet::notify(
            Components::GameManager(GameManager::GameAttribChange),
            AttributesChange {
                id: self.id,
                attributes: &attributes,
            },
        );
        let data = &mut *self.data.write().await;
        data.attributes.extend(attributes);
        self.push_all(&packet).await;
    }

    /// Updates all the client details for the provided session.
    /// Tells each client to send session updates to the session
    /// and the session to send them as well.
    ///
    /// `session` The session to update for
    async fn update_clients(&self, player: &GamePlayer) {
        debug!("Updating clients with new session details");
        let players = &*self.players.read().await;

        let futures = players
            .iter()
            .map(|value| value.exchange_update(player))
            .collect::<Vec<_>>();

        let _ = futures::future::join_all(futures).await;
    }

    /// Retrieves the number of players currently in this game
    async fn player_count(&self) -> usize {
        let players = &*self.players.read().await;
        players.len()
    }

    /// Checks whether the game is full or not by checking
    /// the next slot value is less than the maximum players
    pub async fn is_joinable(&self) -> bool {
        let next_slot = *self.next_slot.read().await;
        next_slot < Self::MAX_PLAYERS
    }

    /// Checks whether the provided session is a player in this game
    ///
    /// `session` The session to check for
    async fn is_player_sid(&self, sid: SessionID) -> bool {
        let players = &*self.players.read().await;
        players.iter().any(|value| value.session_id == sid)
    }

    /// Checks whether this game contains a player with the provided
    /// player ID
    ///
    /// `pid` The player ID
    async fn is_player_pid(&self, pid: PlayerID) -> bool {
        let players = &*self.players.read().await;
        players.iter().any(|value| value.player_id == pid)
    }

    /// Attempts to find a player matching the provided session id then
    /// removing it from the players list returning the index of the
    /// value and the value itself
    ///
    /// `sid` The session ID of the player to take
    async fn take_player_sid(&self, sid: SessionID) -> Option<(usize, GamePlayer)> {
        let players = &mut *self.players.write().await;
        let index = players.iter().position(|value| value.session_id == sid)?;
        Some((index, players.remove(index)))
    }

    /// Attempts to find a player matching the provided player id then
    /// removing it from the players list returning the index of the value
    /// and the value itself
    ///
    /// `pid` The player ID of the player to take
    async fn take_player_pid(&self, pid: PlayerID) -> Option<(usize, GamePlayer)> {
        let players = &mut *self.players.write().await;
        let index = players.iter().position(|value| value.player_id == pid)?;
        let player = players.remove(index);
        Some((index, player))
    }

    pub async fn aquire_slot(&self) -> usize {
        let next_slot = &mut *self.next_slot.write().await;
        let slot = *next_slot;
        *next_slot += 1;
        slot
    }

    pub async fn release_slot(&self) {
        let next_slot = &mut *self.next_slot.write().await;
        *next_slot -= 1;
    }

    /// Adds the provided player to this game
    ///
    /// `session` The session to add
    pub async fn add_player(&self, mut player: GamePlayer) {
        let slot = self.aquire_slot().await;
        player.game_id = self.id;

        self.notify_player_joining(&player, slot).await;
        self.update_clients(&player).await;
        self.notify_game_setup(&player, slot).await;

        player.set_game(Some(self.id)).await;

        let packet = player.create_set_session();
        self.push_all(&packet).await;

        {
            let players = &mut *self.players.write().await;
            players.push(player);
        }

        debug!("Adding player complete");
    }

    /// Notifies all the players in the game that a new player has
    /// joined the game.
    async fn notify_player_joining(&self, player: &GamePlayer, slot: GameSlot) {
        if slot == 0 {
            return;
        }
        let packet = Packet::notify(
            Components::GameManager(GameManager::PlayerJoining),
            PlayerJoining { slot, player },
        );
        self.push_all(&packet).await;
        player.push(packet).await;
    }

    /// Notifies the provided player that the game has been setup and
    /// is ready for them to attempt to join.
    ///
    /// `session` The session to notify
    /// `slot`    The slot the player is joining into
    async fn notify_game_setup(&self, player: &GamePlayer, slot: GameSlot) {
        let players = &*self.players.read().await;
        let game_data = &*self.data.read().await;

        let ty = match slot {
            0 => GameDetailsType::Created,
            _ => GameDetailsType::Joined,
        };

        let packet = Packet::notify(
            Components::GameManager(GameManager::GameSetup),
            GameDetails {
                id: self.id,
                players,
                game_data,
                player,
                ty,
            },
        );

        player.push(packet).await;
    }

    /// Sets the state for the provided session notifying all
    /// the players that the players state has changed.
    ///
    /// `session` The session to change the state of
    /// `state`   The new state value
    async fn set_player_state(&self, session: SessionID, state: PlayerState) {
        let player_id = {
            let players = &mut *self.players.write().await;
            let Some(player) = players.iter_mut().find(|value| value.session_id == session) else {
                return;
            };
            player.state = state;
            player.player_id
        };

        let packet = Packet::notify(
            Components::GameManager(GameManager::GamePlayerStateChange),
            PlayerStateChange {
                gid: self.id,
                pid: player_id,
                state,
            },
        );
        self.push_all(&packet).await;
    }

    /// Modifies the psudo admin list this list doesn't actually exist in
    /// our implementation but we still need to tell the clients these
    /// changes.
    ///
    /// `target`    The player to target for the admin list
    /// `operation` Whether to add or remove the player from the admin list
    async fn modify_admin_list(&self, target: PlayerID, operation: AdminListOperation) {
        let host_id = {
            let players = &*self.players.read().await;
            let Some(host) = players.first() else {
                return;
            };
            host.player_id
        };
        let packet = Packet::notify(
            Components::GameManager(GameManager::AdminListChange),
            AdminListChange {
                game_id: self.id,
                player_id: target,
                operation,
                host_id,
            },
        );
        self.push_all(&packet).await;
    }

    /// Handles updating a mesh connection between two targets. If the target
    /// that the mesh was connected to was a player in the game then the
    /// joining was complete and on_join_complete is processed.
    ///
    /// `session` The session updating its mesh connection
    /// `target`  The pid of the connected target
    pub async fn update_mesh_connection(&self, session: SessionID, target: PlayerID) {
        debug!("Updating mesh connection");
        if self.is_player_sid(session).await && self.is_player_pid(target).await {
            self.set_player_state(session, PlayerState::Connected).await;
            self.on_join_complete(session).await;
            debug!("Connected player to game")
        } else {
            self.set_player_state(session, PlayerState::Connecting)
                .await;
            debug!("Disconnected mesh")
        }
    }

    /// Handles informing the other players in the game when a player joining
    /// is complete (After the mesh connection is updated) and modifies the
    /// admin list to include the newly added session
    ///
    /// `session` The session that completed joining
    async fn on_join_complete(&self, session: SessionID) {
        let players = &*self.players.read().await;
        let Some(player) = players.iter().find(|value| value.session_id == session) else {
            return;
        };
        let packet = Packet::notify(
            Components::GameManager(GameManager::PlayerJoinCompleted),
            JoinComplete {
                game_id: self.id,
                player_id: player.player_id,
            },
        );
        self.push_all(&packet).await;
        self.modify_admin_list(player.player_id, AdminListOperation::Add)
            .await;
    }

    /// Attempts to remove a player by its player ID
    /// this function is used to remove players through
    /// the packet system.
    ///
    /// `pid` The player id of the player to remove
    pub async fn remove_by_pid(&self, pid: PlayerID, reason: RemoveReason) {
        let Some((slot, player)) = self.take_player_pid(pid).await else {
            warn!(
                "Attempted to remove player that wasn't in game (PID: {}, GID: {})",
                pid, self.id
            );
            return;
        };
        self.on_player_removed(player, slot, reason).await;
    }

    /// Attempts to remove a player by its session ID
    /// this function is used to remove players that
    /// have been released or otherwise no longer exist
    ///
    /// `sid` The session ID of the player to remove
    pub async fn remove_by_sid(&self, sid: SessionID) {
        let Some((slot, player)) = self.take_player_sid(sid).await else {
            warn!(
                "Attempted to remove session that wasn't in game (SID: {}, GID: {})",
                sid, self.id
            );
            return;
        };
        self.on_player_removed(player, slot, RemoveReason::Generic)
            .await;
    }

    /// Runs the actions after a player was removed takes the
    /// player itself and the slot the player was in before
    /// it was removed.
    ///
    /// `player` The player that was removed
    /// `slot`   The slot the player used to be in
    async fn on_player_removed(&self, player: GamePlayer, slot: GameSlot, reason: RemoveReason) {
        player.set_game(None).await;
        self.notify_player_removed(&player, reason).await;
        self.notify_fetch_data(&player).await;
        self.modify_admin_list(player.player_id, AdminListOperation::Remove)
            .await;
        debug!(
            "Removed player from game (PID: {}, GID: {})",
            player.player_id, self.id
        );
        // If the player was in the host slot
        if slot == 0 {
            self.try_migrate_host().await;
        }
        self.release_slot().await;
    }

    /// Notifies all the session and the removed session that a
    /// session was removed from the game.
    ///
    /// `player`    The player that was removed
    /// `player_id` The player ID of the removed player
    async fn notify_player_removed(&self, player: &GamePlayer, reason: RemoveReason) {
        let packet = Packet::notify(
            Components::GameManager(GameManager::PlayerRemoved),
            PlayerRemoved {
                game_id: self.id,
                player_id: player.player_id,
                reason,
            },
        );
        self.push_all(&packet).await;
        player.push(packet).await;
    }

    /// Notifies all the sessions in the game to fetch the player data
    /// for the provided session and the session to fetch the extended
    /// data for all the other sessions. Will early return if there
    /// are no players left.
    ///
    /// `session`   The session to update with the other clients
    /// `player_id` The player id of the session to update
    async fn notify_fetch_data(&self, player: &GamePlayer) {
        let players = &*self.players.read().await;
        let removed_packet = Packet::notify(
            Components::UserSessions(UserSessions::FetchExtendedData),
            FetchExtendedData {
                player_id: player.player_id,
            },
        );

        let player_packets = players
            .iter()
            .map(|value| {
                Packet::notify(
                    Components::UserSessions(UserSessions::FetchExtendedData),
                    FetchExtendedData {
                        player_id: value.player_id,
                    },
                )
            })
            .collect::<Vec<_>>();

        join!(
            self.push_all(&removed_packet),
            player.push_all(player_packets)
        );
    }

    /// Attempts to migrate the host of this game if there are still players
    /// left in the game.
    async fn try_migrate_host(&self) {
        let players = &*self.players.read().await;
        let Some(new_host) = players.first() else { return; };

        self.set_state(GameState::HostMigration).await;
        debug!("Starting host migration (GID: {})", self.id);
        self.notify_migrate_start(new_host).await;
        self.set_state(GameState::InGame).await;
        self.notify_migrate_finish().await;
        self.update_clients(new_host).await;

        debug!("Finished host migration (GID: {})", self.id);
    }

    /// Notifies all the sessions in this game that host migration has
    /// begun.
    ///
    /// `new_host` The session that is being migrated to host
    async fn notify_migrate_start(&self, new_host: &GamePlayer) {
        let packet = Packet::notify(
            Components::GameManager(GameManager::HostMigrationStart),
            HostMigrateStart {
                game_id: self.id,
                host_id: new_host.player_id,
            },
        );
        self.push_all(&packet).await;
    }

    /// Notifies to all sessions that the migration is complete
    async fn notify_migrate_finish(&self) {
        let packet = Packet::notify(
            Components::GameManager(GameManager::HostMigrationFinished),
            HostMigrateFinished { game_id: self.id },
        );
        self.push_all(&packet).await;
    }

    /// Checks if the game has no players in it
    pub async fn is_empty(&self) -> bool {
        self.player_count().await == 0
    }

    /// Releases all the players connected to this game
    /// setting their game state to null and clearing
    /// the players list. This releases any stored
    /// player references.
    pub async fn release(&self) {
        let players = &mut *self.players.write().await;
        let futures = players
            .iter()
            .map(|value| value.set_game(None))
            .collect::<Vec<_>>();
        let _ = futures::future::join_all(futures).await;
        players.clear();
    }
}
