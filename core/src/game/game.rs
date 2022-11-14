use blaze_pk::{codec::Codec, packet::Packet, types::TdfMap};

use log::{debug, warn};
use tokio::{join, sync::RwLock};

use crate::blaze::{
    components::{Components, GameManager, UserSessions},
    session::SessionArc,
};

use super::codec::{
    create_game_setup, AdminListChange, AdminListOperation, AttributesChange, FetchExtendedData,
    HostMigrateFinished, HostMigrateStart, JoinComplete, PlayerJoining, PlayerRemoved,
    PlayerStateChange, SettingChange, StateChange,
};

pub struct Game {
    /// Unique ID for this game
    pub id: u32,
    /// Mutable data for this game
    pub data: RwLock<GameData>,
    /// The list of players in this game
    pub players: RwLock<Vec<SessionArc>>,
}

impl Drop for Game {
    fn drop(&mut self) {
        debug!("Game has been dropped (GID: {})", self.id)
    }
}

/// Attributes map type
pub type AttrMap = TdfMap<String, String>;

/// Structure for storing the mutable portion of
/// the game data
pub struct GameData {
    /// The current game state
    pub state: u16,
    /// The current game setting
    pub setting: u16,
    /// The game attributes
    pub attributes: AttrMap,
}

impl GameData {
    const DEFAULT_STATE: u16 = 0x1;

    fn new(setting: u16, attributes: AttrMap) -> Self {
        Self {
            state: Self::DEFAULT_STATE,
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
    pub fn new(id: u32, attributes: AttrMap, setting: u16) -> Self {
        Self {
            id,
            data: RwLock::new(GameData::new(setting, attributes)),
            players: RwLock::new(Vec::new()),
        }
    }

    /// Writes the provided packet to all connected sessions.
    /// Does not wait for the write to complete just waits for
    /// it to be placed into each sessions write buffers.
    ///
    /// `packet` The packet to write
    async fn write_all(&self, packet: &Packet) {
        let players = &*self.players.read().await;
        let futures = players
            .iter()
            .map(|value| value.write(packet))
            .collect::<Vec<_>>();

        let _ = futures::future::join_all(futures).await;
    }

    /// Writes the provided packet to all the connected sessions and the
    /// provided session as well
    ///
    /// `packet` The packet to write
    /// `and`    The additional session to write to
    async fn write_all_and(&self, packet: &Packet, and: &SessionArc) {
        join!(self.write_all(packet), and.write(packet));
    }

    /// Sends a notification packet to all the connected session
    /// with the provided component and contents
    ///
    /// `component` The packet component
    /// `contents`  The packet contents
    async fn notify_all<C: Codec>(&self, component: Components, contents: &C) {
        let packet = Packet::notify(component, contents);
        self.write_all(&packet).await;
    }

    /// Sets the current game state in the game data and
    /// sends an update notification to all connected clients
    /// notifying them of the changed state
    ///
    /// `state` The new state value
    pub async fn set_state(&self, state: u16) {
        debug!("Updating game state (Value: {state})");
        {
            let data = &mut *self.data.write().await;
            data.state = state;
        }

        self.notify_all(
            Components::GameManager(GameManager::GameStateChange),
            &StateChange { id: self.id, state },
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
            &SettingChange {
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
        let data = &mut *self.data.write().await;
        data.attributes.extend(attributes);
        self.notify_all(
            Components::GameManager(GameManager::GameSettingsChange),
            &AttributesChange {
                id: self.id,
                attributes: &data.attributes,
            },
        )
        .await;
    }

    /// Updates all the client details for the provided session.
    /// Tells each client to send session updates to the session
    /// and the session to send them as well.
    ///
    /// `session` The session to update for
    async fn update_clients(&self, session: &SessionArc) {
        debug!("Updating clients with new session details");
        let players = &*self.players.read().await;

        let futures = players
            .iter()
            .map(|value| value.exchange_update(session))
            .collect::<Vec<_>>();

        let _ = futures::future::join_all(futures).await;
    }

    /// Retrieves the number of players currently in this game
    async fn player_count(&self) -> usize {
        let players = &*self.players.read().await;
        players.len()
    }

    /// Checks whether the game is full or not
    pub async fn is_joinable(&self) -> bool {
        self.player_count().await < Self::MAX_PLAYERS
    }

    /// Checks whether the provided session is a player in this game
    ///
    /// `session` The session to check for
    async fn is_player(&self, session: &SessionArc) -> bool {
        let players = &*self.players.read().await;
        players.iter().any(|value| value.id == session.id)
    }

    /// Checks whether this game contains a player with the provided
    /// player ID
    ///
    /// `pid` The player ID
    async fn is_player_pid(&self, pid: u32) -> bool {
        let players = &*self.players.read().await;
        for player in players {
            let player_data = player.data.read().await;
            if let Some(player) = player_data.player.as_ref() {
                if player.id == pid {
                    return true;
                }
            }
        }
        false
    }

    /// Attempts to find a player matching the provided session id then
    /// removing it from the players list returning the index of the
    /// value and the value itself
    ///
    /// `sid` The session ID of the player to take
    async fn take_player_sid(&self, sid: u32) -> Option<(usize, SessionArc)> {
        let players = &mut *self.players.write().await;
        let index = players.iter().position(|value| value.id == sid)?;
        Some((index, players.remove(index)))
    }

    /// Attempts to find a player matching the provided player id then
    /// removing it from the players list returning the index of the value
    /// and the value itself
    ///
    /// `pid` The player ID of the player to take
    async fn take_player_pid(&self, pid: u32) -> Option<(usize, SessionArc)> {
        let players = &mut *self.players.write().await;
        let mut target_index = None;

        for (index, player) in players.iter().enumerate() {
            let player_data = &*player.data.read().await;
            if let Some(player) = player_data.player.as_ref() {
                if player.id == pid {
                    target_index = Some(index);
                    break;
                }
            }
        }

        let index = target_index?;
        let player = players.remove(index);
        Some((index, player))
    }

    /// Adds the provided player to this game
    ///
    /// `session` The session to add
    pub async fn add_player(&self, session: &SessionArc) {
        let slot = self.player_count().await;

        self.notify_player_joining(session, slot).await;
        self.update_clients(session).await;

        {
            let players = &mut *self.players.write().await;
            players.push(session.clone());
        }

        session.set_game(self.id).await;

        self.notify_game_setup(session, slot).await;
        self.set_session_all(session).await;

        debug!("Adding player complete");
    }

    /// Notifies the provided player that the game has been setup and
    /// is ready for them to attempt to join.
    ///
    /// `session` The session to notify
    /// `slot`    The slot the player is joining into
    async fn notify_game_setup(&self, session: &SessionArc, slot: usize) {
        let is_host = slot == 0;
        let packet = create_game_setup(self, is_host, session).await;
        session.write(&packet).await;
    }

    /// Sends the set session packet for the provided session to all
    /// the other sessions in this game
    async fn set_session_all(&self, session: &SessionArc) {
        let packet = session.create_set_session().await;
        join!(self.write_all(&packet), session.write(&packet));
    }

    /// Notifies all the players in the game that a new player has
    /// joined the game.
    async fn notify_player_joining(&self, session: &SessionArc, slot: usize) {
        if slot == 0 {
            return;
        }
        let session_data = &*session.data.read().await;
        let packet = Packet::notify(
            Components::GameManager(GameManager::PlayerJoining),
            &PlayerJoining {
                id: self.id,
                slot,
                session: &session_data,
            },
        );
        self.write_all_and(&packet, session).await;
    }

    /// Sets the state for the provided session notifying all
    /// the players that the players state has changed.
    ///
    /// `session` The session to change the state of
    /// `state`   The new state value
    async fn set_player_state(&self, session: &SessionArc, state: u8) {
        let player_id = {
            let session_data = &mut *session.data.write().await;
            session_data.state = state;
            session_data.id_safe()
        };

        let packet = Packet::notify(
            Components::GameManager(GameManager::GamePlayerStateChange),
            &PlayerStateChange {
                gid: self.id,
                pid: player_id,
                state,
            },
        );
        self.write_all(&packet).await;
    }

    /// Modifies the psudo admin list this list doesn't actually exist in
    /// our implementation but we still need to tell the clients these
    /// changes.
    ///
    /// `target`    The player to target for the admin list
    /// `operation` Whether to add or remove the player from the admin list
    async fn modify_admin_list(&self, target: u32, operation: AdminListOperation) {
        let host_id = {
            let players = &*self.players.read().await;
            let Some(host) = players.first() else {
                return;
            };
            host.player_id_safe().await
        };
        let packet = Packet::notify(
            Components::GameManager(GameManager::AdminListChange),
            &AdminListChange {
                game_id: self.id,
                player_id: target,
                operation,
                host_id: host_id,
            },
        );
        self.write_all(&packet).await;
    }

    /// Handles updating a mesh connection between two targets. If the target
    /// that the mesh was connected to was a player in the game then the
    /// joining was complete and on_join_complete is processed.
    ///
    /// `session` The session updating its mesh connection
    /// `target`  The pid of the connected target
    pub async fn update_mesh_connection(&self, session: &SessionArc, target: u32) {
        if self.is_player(session).await && self.is_player_pid(target).await {
            self.set_player_state(session, 4).await;
            self.on_join_complete(session).await;
        } else {
            self.set_player_state(session, 2).await;
        }
    }

    /// Handles informing the other players in the game when a player joining
    /// is complete (After the mesh connection is updated) and modifies the
    /// admin list to include the newly added session
    ///
    /// `session` The session that completed joining
    async fn on_join_complete(&self, session: &SessionArc) {
        let player_id = session.player_id_safe().await;
        let packet = Packet::notify(
            Components::GameManager(GameManager::PlayerJoinCompleted),
            &JoinComplete {
                game_id: self.id,
                player_id,
            },
        );
        self.write_all(&packet).await;
        self.modify_admin_list(player_id, AdminListOperation::Add)
            .await;
    }

    /// Attempts to remove a player by its player ID
    /// this function is used to remove players through
    /// the packet system.
    ///
    /// `pid` The player id of the player to remove
    pub async fn remove_by_pid(&self, pid: u32) {
        let Some((slot, player)) = self.take_player_pid(pid).await else {
            warn!(
                "Attempted to remove player that wasn't in game (PID: {}, GID: {})",
                pid, self.id
            );
            return;
        };
        self.on_player_removed(player, slot).await;
    }

    /// Attempts to remove a player by its session ID
    /// this function is used to remove players that
    /// have been released or otherwise no longer exist
    ///
    /// `sid` The session ID of the player to remove
    pub async fn remove_by_sid(&self, sid: u32) {
        let Some((slot, player)) = self.take_player_sid(sid).await else {
            warn!(
                "Attempted to remove session that wasn't in game (SID: {}, GID: {})",
                sid, self.id
            );
            return;
        };
        self.on_player_removed(player, slot).await;
    }

    /// Runs the actions after a player was removed takes the
    /// player itself and the slot the player was in before
    /// it was removed.
    ///
    /// `player` The player that was removed
    /// `slot`   The slot the player used to be in
    async fn on_player_removed(&self, player: SessionArc, slot: usize) {
        player.clear_game().await;
        let player_id = player.player_id_safe().await;
        self.notify_player_removed(&player, player_id).await;
        self.modify_admin_list(player_id, AdminListOperation::Remove)
            .await;
        self.notify_fetch_data(&player, player_id).await;
        debug!(
            "Removed player from game (PID: {}, GID: {})",
            player_id, self.id
        );

        // If the player was in the host slot
        if slot == 0 {
            self.try_migrate_host(&player).await;
        }
    }

    /// Notifies all the session and the removed session that a
    /// session was removed from the game.
    ///
    /// `player`    The player that was removed
    /// `player_id` The player ID of the removed player
    async fn notify_player_removed(&self, player: &SessionArc, player_id: u32) {
        let packet = Packet::notify(
            Components::GameManager(GameManager::PlayerRemoved),
            &PlayerRemoved {
                game_id: self.id,
                player_id,
            },
        );
        self.write_all_and(&packet, player).await;
    }

    /// Notifies all the sessions in the game to fetch the player data
    /// for the provided session and the session to fetch the extended
    /// data for all the other sessions. Will early return if there
    /// are no players left.
    ///
    /// `session`   The session to update with the other clients
    /// `player_id` The player id of the session to update
    async fn notify_fetch_data(&self, session: &SessionArc, player_id: u32) {
        let player_ids = {
            let players = &*self.players.read().await;
            if players.is_empty() {
                return;
            }

            let futures = players
                .iter()
                .map(|value| value.player_id_safe())
                .collect::<Vec<_>>();
            futures::future::join_all(futures).await
        };

        let removed_packet = Packet::notify(
            Components::UserSessions(UserSessions::FetchExtendedData),
            &FetchExtendedData { id: player_id },
        );

        let player_packets = player_ids
            .into_iter()
            .map(|id| {
                Packet::notify(
                    Components::UserSessions(UserSessions::FetchExtendedData),
                    &FetchExtendedData { id },
                )
            })
            .collect::<Vec<_>>();

        join!(
            self.write_all(&removed_packet),
            session.write_all(&player_packets)
        );
    }

    /// Attempts to migrate the host of this game if there are still players
    /// left in the game.
    async fn try_migrate_host(&self, old_host: &SessionArc) {
        let players = &*self.players.read().await;
        let Some(new_host) = players.first() else { return; };

        debug!("Starting host migration (GID: {})", self.id);
        self.notify_migrate_start(new_host).await;
        self.set_state(0x82).await;
        self.notify_migrate_finish().await;
        self.update_clients(new_host).await;
        self.set_session_all(old_host).await;
        debug!("Finished host migration (GID: {})", self.id);
    }

    /// Notifies all the sessions in this game that host migration has
    /// begun.
    ///
    /// `new_host` The session that is being migrated to host
    async fn notify_migrate_start(&self, new_host: &SessionArc) {
        let host_id = new_host.player_id_safe().await;
        let packet = Packet::notify(
            Components::GameManager(GameManager::HostMigrationStart),
            &HostMigrateStart {
                game_id: self.id,
                host_id,
            },
        );
        self.write_all(&packet).await;
    }

    /// Notifies to all sessions that the migration is complete
    async fn notify_migrate_finish(&self) {
        let packet = Packet::notify(
            Components::GameManager(GameManager::HostMigrationFinished),
            &HostMigrateFinished { game_id: self.id },
        );
        self.write_all(&packet).await;
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
            .map(|value| value.clear_game())
            .collect::<Vec<_>>();
        let _ = futures::future::join_all(futures).await;
        players.clear();
    }
}
