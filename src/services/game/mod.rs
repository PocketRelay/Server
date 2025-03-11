use self::rules::RuleSet;
use crate::{
    config::Config,
    database::entities::Player,
    session::{
        data::NetData,
        models::{
            game_manager::{
                AdminListChange, AdminListOperation, AttributesChange, GameSettings,
                GameSetupContext, GameSetupResponse, GameState, GetGameDetails,
                HostMigrateFinished, HostMigrateStart, JoinComplete, PlayerJoining,
                PlayerNetConnectionStatus, PlayerRemoved, PlayerState, PlayerStateChange,
                RemoveReason, ReportingIdChange, SettingChange, SlotType, StateChange,
                UNSPECIFIED_TEAM_INDEX,
            },
            util::LOCALE_NZ,
            NetworkAddress,
        },
        packet::Packet,
        router::RawBlaze,
        SessionLink, WeakSessionLink,
    },
    utils::{
        components::game_manager,
        types::{GameID, PlayerID},
    },
};
use chrono::{DateTime, Utc};
use log::{debug, warn};
use parking_lot::RwLock;
use rand::RngCore;
use std::sync::{Arc, Weak};
use store::Games;
use tdf::{ObjectId, TdfMap, TdfSerializer};

use super::tunnel::TunnelService;

pub mod matchmaking;
pub mod rules;
pub mod snapshot;
pub mod store;

pub type GameRef = Arc<RwLock<Game>>;
pub type WeakGameRef = Weak<RwLock<Game>>;

pub trait GameAddPlayerExt {
    fn add_player(
        &self,
        tunnel_service: &TunnelService,
        config: &Config,

        player: GamePlayer,
        session: SessionLink,
        context: GameSetupContext,
    );
}

impl GameAddPlayerExt for GameRef {
    fn add_player(
        &self,
        tunnel_service: &TunnelService,
        config: &Config,

        player: GamePlayer,
        session: SessionLink,
        context: GameSetupContext,
    ) {
        // Add the player to the game
        let (game_id, index) = {
            let game = &mut *self.write();
            let slot = game.add_player(player, context, config);
            (game.id, slot)
        };

        // Allocate tunnel if supported by client
        if let Some(association) = session.data.get_association() {
            tunnel_service.associate_pool(association, game_id, index as u8);
        }

        // Update the player current game
        session.data.set_game(game_id, Arc::downgrade(self));
    }
}

/// Game service running within the server
pub struct Game {
    /// Unique ID for this game
    pub id: GameID,
    /// The current game state
    pub state: GameState,
    /// The current game setting
    pub settings: GameSettings,
    /// The game attributes
    pub attributes: AttrMap,
    /// When the game was started
    pub created_at: DateTime<Utc>,
    /// Players currently in the game
    pub players: Vec<GamePlayer>,
    /// ID used when reporting game results
    pub reporting_id: u64,
    /// Randomness seed used by players
    pub seed: u32,

    /// Services access
    pub games_store: Arc<Games>,
    /// Access to the tunneling service
    pub tunnel_service: Arc<TunnelService>,
}

/// Attributes map type
pub type AttrMap = TdfMap<String, String>;

/// Player structure containing details and state for a player
/// within a game
pub struct GamePlayer {
    /// Session player
    pub player: Arc<Player>,
    /// Weak reference to the associated session
    pub link: WeakSessionLink,
    /// The mesh state of the player
    pub state: PlayerState,
    /// Snapshot of the player data when they entered the game
    /// (Used at end-game to determine the outcome of the game and the characters/weapons used)
    pub data_snapshot: GamePlayerPlayerDataSnapshot,
}

#[derive(Clone)]
pub struct GamePlayerPlayerDataSnapshot {
    pub data: Vec<(String, String)>,
}

impl GamePlayer {
    /// Creates a new game player structure with the provided player
    /// details
    ///
    /// `player` The session player
    /// `net`    The player networking details
    /// `addr`   The session address
    pub fn new(
        player: Arc<Player>,
        link: WeakSessionLink,
        data_snapshot: GamePlayerPlayerDataSnapshot,
    ) -> Self {
        Self {
            player,
            link,
            state: PlayerState::ActiveConnecting,
            data_snapshot,
        }
    }

    pub fn net(&self) -> Option<Arc<NetData>> {
        let session = self.link.upgrade()?;
        session.data.net()
    }

    pub fn network_address(&self) -> NetworkAddress {
        match self.net() {
            Some(net) => net.addr.clone(),
            None => NetworkAddress::Unset,
        }
    }

    pub fn try_clear_game(&self) {
        if let Some(link) = self.link.upgrade() {
            link.data.clear_game_gm();
        }
    }

    pub fn try_subscribe(&self, player_id: PlayerID, subscriber: WeakSessionLink) {
        if let Some(link) = self.link.upgrade() {
            link.data.add_subscriber(player_id, subscriber);
        }
    }

    pub fn try_unsubscribe(&self, player_id: PlayerID) {
        if let Some(link) = self.link.upgrade() {
            link.data.remove_subscriber(player_id);
        }
    }

    #[inline]
    pub fn notify(&self, packet: Packet) {
        let session = match self.link.upgrade() {
            Some(value) => value,
            // Session is already closed
            None => return,
        };

        session.tx.notify(packet)
    }

    pub fn encode<S: TdfSerializer>(&self, game_id: GameID, slot: usize, w: &mut S) {
        w.group_body(|w| {
            // Custom data
            w.tag_blob_empty(b"BLOB");
            // External ID
            w.tag_u8(b"EXID", 0);
            // Game ID
            w.tag_owned(b"GID", game_id);
            // Account locale
            w.tag_u32(b"LOC", LOCALE_NZ);
            // Player name
            w.tag_str(b"NAME", &self.player.display_name);
            // Player ID
            w.tag_u32(b"PID", self.player.id);
            // Player network data
            w.tag_ref(b"PNET", &self.network_address());
            // Slot ID
            w.tag_owned(b"SID", slot);
            // Slot type
            w.tag_alt(b"SLOT", SlotType::PublicParticipant);
            // Player state
            w.tag_ref(b"STAT", &self.state);
            // Team index
            w.tag_u16(b"TIDX", UNSPECIFIED_TEAM_INDEX);
            // Unix millisecond timestamp of the player joined the game in
            w.tag_u8(b"TIME", 0);
            // User group ID
            w.tag_alt(b"UGID", ObjectId::new_raw(0, 0, 0));
            // Player session ID
            w.tag_u32(b"UID", self.player.id);
        });
    }
}

/// Different results for checking if a game is
/// joinable
pub enum GameJoinableState {
    /// Game is currently joinable
    Joinable,
    /// Game is full
    Full,
    /// The game doesn't match the provided rules
    NotMatch,
    /// The game is stopping
    Stopping,
}

impl Game {
    /// Constant for the maximum number of players allowed in
    /// a game at one time. Used to determine a games full state
    pub const MAX_PLAYERS: usize = 4;

    pub fn new(
        id: GameID,
        attributes: AttrMap,
        settings: GameSettings,
        games_store: Arc<Games>,
        tunnel_service: Arc<TunnelService>,
    ) -> Game {
        let mut rng = rand::rng();
        let seed = rng.next_u32();

        Game {
            id,
            attributes,
            settings,
            state: Default::default(),
            players: Default::default(),
            created_at: Utc::now(),
            reporting_id: 18014398695176361,
            seed,
            games_store,
            tunnel_service,
        }
    }

    /// Handle a "replay", game resetting its state back to pre-game
    /// updates game reporting id
    pub fn replay(&mut self) {
        self.set_state(GameState::PreGame);

        // TODO: Rotate game reporting ID to a new ID
        self.set_game_reporting_id(18014398695176361);
    }

    pub fn game_data(&self) -> RawBlaze {
        let data = GetGameDetails { game: self };
        data.into()
    }

    pub fn get_players_with_state(&self) -> Vec<(Arc<Player>, GamePlayerPlayerDataSnapshot)> {
        self.players
            .iter()
            .map(|value| (value.player.clone(), value.data_snapshot.clone()))
            .collect()
    }

    pub fn add_player(
        &mut self,
        player: GamePlayer,
        context: GameSetupContext,
        config: &Config,
    ) -> usize {
        let slot = self.players.len();

        // Update other players with the client details
        self.add_user_sub(&player);

        // Notify other players of the joining player
        self.notify_all(Packet::notify(
            game_manager::COMPONENT,
            game_manager::PLAYER_JOINING,
            PlayerJoining {
                slot,
                player: &player,
                game_id: self.id,
            },
        ));

        self.players.push(player);

        // Get the player that was just added
        let player = self
            .players
            .last()
            .expect("Expected inserted player was missing");

        // Send the player the game setup details
        player.notify(Packet::notify(
            game_manager::COMPONENT,
            game_manager::GAME_SETUP,
            GameSetupResponse {
                game: self,
                context,
                config,
            },
        ));

        slot
    }

    pub fn add_admin_player(&mut self, target_id: PlayerID) {
        // Add the player to the admin list
        self.modify_admin_list(target_id, AdminListOperation::Add);
    }

    pub fn is_host_player(&self, player_id: PlayerID) -> bool {
        self.players
            .first()
            .is_some_and(|host| host.player.id == player_id)
    }

    pub fn update_mesh(&mut self, target_id: PlayerID, status: PlayerNetConnectionStatus) {
        // We only care about a connected state
        if !matches!(status, PlayerNetConnectionStatus::Connected) {
            return;
        }

        // Obtain the target player
        let target_slot = match self
            .players
            .iter_mut()
            .find(|slot| slot.player.id == target_id)
        {
            Some(value) => value,
            None => {
                debug!(
                    "Unable to find player to update mesh state for (PID: {} GID: {})",
                    target_id, self.id
                );
                return;
            }
        };

        // Mark the player as connected and update the state for all users
        target_slot.state = PlayerState::ActiveConnected;
        self.notify_all(Packet::notify(
            game_manager::COMPONENT,
            game_manager::GAME_PLAYER_STATE_CHANGE,
            PlayerStateChange {
                gid: self.id,
                pid: target_id,
                state: PlayerState::ActiveConnected,
            },
        ));

        // Notify all players that the player has completely joined
        self.notify_all(Packet::notify(
            game_manager::COMPONENT,
            game_manager::PLAYER_JOIN_COMPLETED,
            JoinComplete {
                game_id: self.id,
                player_id: target_id,
            },
        ));
    }

    pub fn remove_player(&mut self, id: u32, reason: RemoveReason) {
        // Already empty game handling
        if self.players.is_empty() {
            self.stop();
            return;
        }

        // Find the player index
        let index = self.players.iter().position(|v| v.player.id == id);

        let index = match index {
            Some(value) => value,
            None => return,
        };

        // Remove the tunnel
        self.tunnel_service.dissociate_pool(self.id, index as u8);

        // Remove the player
        let player = self.players.remove(index);

        // Clear current game of this player
        player.try_clear_game();

        // Update the other players
        self.notify_player_removed(&player, reason);
        self.rem_user_sub(&player);
        self.modify_admin_list(player.player.id, AdminListOperation::Remove);

        debug!(
            "Removed player from game (PID: {}, GID: {})",
            player.player.id, self.id
        );

        drop(player);

        // If the player was in the host slot attempt migration
        if index == 0 {
            self.try_migrate_host();
        }

        if self.players.is_empty() {
            // Game is empty stop it
            self.stop();
        }
    }

    fn stop(&mut self) {
        // Mark the game as stopping
        self.state = GameState::Destructing;

        if !self.players.is_empty() {
            warn!("Game {} was stopped with players still present", self.id);
        }

        // Remove the stopping game
        self.games_store.remove_by_id(self.id);
    }

    pub fn joinable_state(&self, rule_set: Option<&RuleSet>) -> GameJoinableState {
        if let GameState::Destructing = self.state {
            return GameJoinableState::Stopping;
        }

        // Handle full game
        if self.players.len() >= Self::MAX_PLAYERS {
            return GameJoinableState::Full;
        }

        // Check rule set matches
        if let Some(rule_set) = rule_set {
            if !rule_set.matches(&self.attributes) {
                return GameJoinableState::NotMatch;
            }
        }

        GameJoinableState::Joinable
    }

    /// Writes the provided packet to all connected sessions.
    /// Does not wait for the write to complete just waits for
    /// it to be placed into each sessions write buffers.
    ///
    /// `packet` The packet to write
    fn notify_all(&self, packet: Packet) {
        self.players
            .iter()
            .for_each(|value| value.notify(packet.clone()));
    }

    pub fn set_state(&mut self, state: GameState) {
        self.state = state;

        debug!("Updated game state (Value: {:?})", &state);

        self.notify_all(Packet::notify(
            game_manager::COMPONENT,
            game_manager::GAME_STATE_CHANGE,
            StateChange { id: self.id, state },
        ));
    }

    pub fn set_game_reporting_id(&mut self, reporting_id: u64) {
        self.reporting_id = reporting_id;

        debug!("Updated game reporting id (Value: {:?})", &reporting_id);

        self.notify_all(Packet::notify(
            game_manager::COMPONENT,
            game_manager::GAME_REPORTING_ID_CHANGE,
            ReportingIdChange {
                id: self.id,
                grid: reporting_id,
            },
        ));
    }

    pub fn set_settings(&mut self, settings: GameSettings) {
        self.settings = settings;

        debug!("Updated game setting (Value: {:?})", &settings);

        self.notify_all(Packet::notify(
            game_manager::COMPONENT,
            game_manager::GAME_SETTINGS_CHANGE,
            SettingChange {
                id: self.id,
                settings,
            },
        ));
    }

    pub fn set_attributes(&mut self, attributes: AttrMap) {
        let packet = Packet::notify(
            game_manager::COMPONENT,
            game_manager::GAME_ATTRIB_CHANGE,
            AttributesChange {
                id: self.id,
                attributes: &attributes,
            },
        );

        self.attributes.insert_presorted(attributes.into_inner());

        debug!("Updated game attributes");

        self.notify_all(packet);
    }

    /// Creates a subscription between all the users and the the target player
    fn add_user_sub(&self, target: &GamePlayer) {
        debug!("Adding user subscriptions");

        // Subscribe all the clients to each other
        self.players
            .iter()
            .filter(|other| other.player.id != target.player.id)
            .for_each(|other| {
                target.try_subscribe(other.player.id, other.link.clone());
                other.try_subscribe(target.player.id, target.link.clone());
            });
    }

    /// Notifies the provided player and all other players
    /// in the game that they should remove each other from
    /// their player data list
    fn rem_user_sub(&self, target: &GamePlayer) {
        debug!("Removing user subscriptions");

        // Unsubscribe all the clients from each other
        self.players
            .iter()
            .filter(|other| other.player.id != target.player.id)
            .for_each(|other| {
                target.try_unsubscribe(other.player.id);
                other.try_unsubscribe(target.player.id);
            });
    }

    /// Modifies the pseudo admin list this list doesn't actually exist in
    /// our implementation but we still need to tell the clients these
    /// changes.
    ///
    /// `target`    The player to target for the admin list
    /// `operation` Whether to add or remove the player from the admin list
    fn modify_admin_list(&self, target: PlayerID, operation: AdminListOperation) {
        let host = match self.players.first() {
            Some(value) => value,
            None => return,
        };

        self.notify_all(Packet::notify(
            game_manager::COMPONENT,
            game_manager::ADMIN_LIST_CHANGE,
            AdminListChange {
                game_id: self.id,
                player_id: target,
                operation,
                host_id: host.player.id,
            },
        ));
    }

    /// Notifies all the session and the removed session that a
    /// session was removed from the game.
    ///
    /// `player`    The player that was removed
    /// `player_id` The player ID of the removed player
    fn notify_player_removed(&self, player: &GamePlayer, reason: RemoveReason) {
        let packet = Packet::notify(
            game_manager::COMPONENT,
            game_manager::PLAYER_REMOVED,
            PlayerRemoved {
                cntx: 0,
                game_id: self.id,
                player_id: player.player.id,
                reason,
            },
        );
        self.notify_all(packet.clone());
        player.notify(packet);
    }

    /// Attempts to migrate the host of this game if there are still players
    /// left in the game.
    fn try_migrate_host(&mut self) {
        // TODO: With more than one player this fails

        // Obtain the new host player
        let host_id = match self.players.first().map(|player| player.player.id) {
            Some(value) => value,
            None => return,
        };

        debug!("Starting host migration (GID: {})", self.id);

        // Start host migration
        self.set_state(GameState::Migrating);
        self.notify_all(Packet::notify(
            game_manager::COMPONENT,
            game_manager::HOST_MIGRATION_START,
            HostMigrateStart {
                game_id: self.id,
                host_id,
                pmig: 2,
                slot: 0,
            },
        ));

        // Finished host migration
        self.set_state(GameState::InGame);
        self.notify_all(Packet::notify(
            game_manager::COMPONENT,
            game_manager::HOST_MIGRATION_FINISHED,
            HostMigrateFinished { game_id: self.id },
        ));

        debug!("Finished host migration (GID: {})", self.id);
    }
}

impl Drop for Game {
    fn drop(&mut self) {
        debug!("Game is stopped (GID: {})", self.id);
    }
}
