use self::{manager::GameManager, rules::RuleSet};
use crate::{
    database::entities::Player,
    session::{
        models::game_manager::{
            AdminListChange, AdminListOperation, AttributesChange, GameSettings, GameSetupContext,
            GameSetupResponse, GameState, GetGameDetails, HostMigrateFinished, HostMigrateStart,
            JoinComplete, PlayerJoining, PlayerRemoved, PlayerState, PlayerStateChange,
            RemoveReason, SettingChange, StateChange,
        },
        packet::Packet,
        router::RawBlaze,
        NetData, SessionLink,
    },
    utils::{
        components::game_manager,
        types::{GameID, PlayerID},
    },
};
use log::debug;
use serde::Serialize;
use std::sync::Arc;
use tdf::{ObjectId, TdfMap, TdfSerializer};
use tokio::sync::RwLock;

pub mod manager;
pub mod rules;

pub type GameRef = Arc<RwLock<Game>>;

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

    pub players: Vec<GamePlayer>,

    /// Services access
    pub game_manager: Arc<GameManager>,
}

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
    pub players: Box<[GamePlayerSnapshot]>,
}

/// Attributes map type
pub type AttrMap = TdfMap<String, String>;

/// Player structure containing details and state for a player
/// within a game
pub struct GamePlayer {
    /// Session player
    pub player: Arc<Player>,
    /// Session address
    pub link: SessionLink,
    /// Networking information for the player
    pub net: Arc<NetData>,
    /// The mesh state of the player
    pub state: PlayerState,
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

impl GamePlayer {
    /// Creates a new game player structure with the provided player
    /// details
    ///
    /// `player` The session player
    /// `net`    The player networking details
    /// `addr`   The session address
    pub fn new(player: Arc<Player>, net: Arc<NetData>, link: SessionLink) -> Self {
        Self {
            player,
            link,
            net,
            state: PlayerState::ActiveConnecting,
        }
    }

    pub fn set_game(&self, game: Option<GameID>) {
        let link = self.link.clone();
        tokio::spawn(async move {
            link.set_game(game).await;
        });
    }

    /// Takes a snapshot of the current player state
    /// for serialization
    pub fn snapshot(&self, include_net: bool) -> GamePlayerSnapshot {
        GamePlayerSnapshot {
            player_id: self.player.id,
            display_name: Box::from(self.player.display_name.as_ref()),
            net: if include_net {
                Some(self.net.clone())
            } else {
                None
            },
        }
    }

    pub fn encode<S: TdfSerializer>(&self, game_id: GameID, slot: usize, w: &mut S) {
        w.group_body(|w| {
            w.tag_blob_empty(b"BLOB");
            w.tag_u8(b"EXID", 0);
            w.tag_owned(b"GID", game_id);
            w.tag_u32(b"LOC", 0x64654445);
            w.tag_str(b"NAME", &self.player.display_name);
            w.tag_u32(b"PID", self.player.id);
            w.tag_ref(b"PNET", &self.net.addr);
            w.tag_owned(b"SID", slot);
            w.tag_u8(b"SLOT", 0);
            w.tag_ref(b"STAT", &self.state);
            w.tag_u16(b"TIDX", 0xffff);
            w.tag_u8(b"TIME", 0); /* Unix timestamp in millseconds */
            w.tag_alt(b"UGID", ObjectId::new_raw(0, 0, 0));
            w.tag_u32(b"UID", self.player.id);
        });
    }
}

impl Drop for GamePlayer {
    fn drop(&mut self) {
        // Clear player game when game player is dropped
        self.set_game(None);
    }
}

/// Different results for checking if a game is
/// joinable
pub enum GameJoinableState {
    /// Game is currenlty joinable
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
    const MAX_PLAYERS: usize = 4;

    pub async fn game_data(&self) -> RawBlaze {
        let data = GetGameDetails { game: self };
        data.into()
    }

    pub fn add_player(&mut self, mut player: GamePlayer, context: GameSetupContext) {
        let slot = self.players.len();

        // Player is the host player (They are connected)
        if slot == 0 {
            player.state = PlayerState::ActiveConnected;
        }

        self.players.push(player);

        // Obtain the player that was just added
        let player = self
            .players
            .last()
            .expect("Player was added but is missing from players");

        // Player isn't the host player
        if slot != 0 {
            // Notify other players of the joined player
            self.push_all(&Packet::notify(
                game_manager::COMPONENT,
                game_manager::PLAYER_JOINING,
                PlayerJoining {
                    slot,
                    player,
                    game_id: self.id,
                },
            ));

            // Update other players with the client details
            self.add_user_sub(player.player.id, player.link.clone());
        }

        // Notify the joiner of the game details
        self.notify_game_setup(player, context);

        // Set current game of this player
        player.set_game(Some(self.id));
    }

    pub fn update_mesh(&mut self, id: PlayerID, target: PlayerID, state: PlayerState) {
        if let PlayerState::ActiveConnecting = state {
            // Ensure the target player is in the game
            if !self.players.iter().any(|value| value.player.id == target) {
                return;
            }

            // Find the index of the session player
            let session = self.players.iter_mut().find(|value| value.player.id == id);

            let session = match session {
                Some(value) => value,
                None => return,
            };

            // Update the session state
            session.state = PlayerState::ActiveConnected;

            let player_id = session.player.id;
            let state_change = PlayerStateChange {
                gid: self.id,
                pid: player_id,
                state: session.state,
            };

            // TODO: Move into a "connection complete" function

            // Notify players of the player state change
            self.push_all(&Packet::notify(
                game_manager::COMPONENT,
                game_manager::GAME_PLAYER_STATE_CHANGE,
                state_change,
            ));

            // Notify players of the completed connection
            self.push_all(&Packet::notify(
                game_manager::COMPONENT,
                game_manager::PLAYER_JOIN_COMPLETED,
                JoinComplete {
                    game_id: self.id,
                    player_id,
                },
            ));

            // Add the player to the admin list
            self.modify_admin_list(player_id, AdminListOperation::Add);
        }
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

        // Remove the player
        let player = self.players.remove(index);

        // Set current game of this player
        player.set_game(None);

        // Update the other players
        self.notify_player_removed(&player, reason);
        self.rem_user_sub(player.player.id, player.link.clone());
        self.modify_admin_list(player.player.id, AdminListOperation::Remove);

        debug!(
            "Removed player from game (PID: {}, GID: {})",
            player.player.id, self.id
        );

        // If the player was in the host slot attempt migration
        if index == 0 {
            self.try_migrate_host();
        }

        if self.players.is_empty() {
            // Game is empty stop it
            self.stop();
        }
    }

    pub fn new(
        id: GameID,
        attributes: AttrMap,
        settings: GameSettings,
        game_manager: Arc<GameManager>,
    ) -> Game {
        Game {
            id,
            attributes,
            settings,
            state: Default::default(),
            players: Default::default(),
            game_manager,
        }
    }

    /// Called by the game manager service once this game has been stopped and
    /// removed from the game list
    fn stopped(&mut self) {
        self.state = GameState::Destructing;
        debug!("Game is stopped (GID: {})", self.id);
    }

    fn stop(&self) {
        // Remove the stopping game
        let game_manager = self.game_manager.clone();
        let game_id = self.id;
        tokio::spawn(async move {
            game_manager.remove_game(game_id).await;
        });
    }

    pub fn joinable_state(&self, rule_set: Option<&RuleSet>) -> GameJoinableState {
        if let GameState::Destructing = self.state {
            return GameJoinableState::Stopping;
        }

        // Handle full game
        if self.players.len() >= Self::MAX_PLAYERS {
            return GameJoinableState::Full;
        }

        // Check ruleset matches
        if let Some(rule_set) = rule_set {
            if !rule_set.matches(&self.attributes) {
                return GameJoinableState::NotMatch;
            }
        }

        GameJoinableState::Joinable
    }

    pub fn snapshot(&self, include_net: bool) -> GameSnapshot {
        let players = self
            .players
            .iter()
            .map(|value| value.snapshot(include_net))
            .collect();

        GameSnapshot {
            id: self.id,
            state: self.state,
            setting: self.settings.bits(),
            attributes: self.attributes.clone(),
            players,
        }
    }

    /// Writes the provided packet to all connected sessions.
    /// Does not wait for the write to complete just waits for
    /// it to be placed into each sessions write buffers.
    ///
    /// `packet` The packet to write
    fn push_all(&self, packet: &Packet) {
        self.players
            .iter()
            .for_each(|value| value.link.push(packet.clone()));
    }

    pub fn set_state(&mut self, state: GameState) {
        self.state = state;

        debug!("Updated game state (Value: {:?})", &state);

        self.push_all(&Packet::notify(
            game_manager::COMPONENT,
            game_manager::GAME_STATE_CHANGE,
            StateChange { id: self.id, state },
        ));
    }

    pub fn set_settings(&mut self, settings: GameSettings) {
        self.settings = settings;

        debug!("Updated game setting (Value: {:?})", &settings);

        self.push_all(&Packet::notify(
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

        self.push_all(&packet);
    }

    /// Creates a subscription between all the users and the the target player
    fn add_user_sub(&self, target_id: PlayerID, target_link: SessionLink) {
        debug!("Adding user subscriptions");

        // Subscribe all the clients to eachother
        self.players
            .iter()
            .filter(|other| other.player.id.ne(&target_id))
            .for_each(|other| {
                let other_id = other.player.id;
                let other_link = other.link.clone();
                let target_link = target_link.clone();

                tokio::spawn(async move {
                    target_link
                        .add_subscriber(other_id, other_link.clone())
                        .await;
                    other_link
                        .add_subscriber(target_id, target_link.clone())
                        .await;
                });
            });
    }

    /// Notifies the provided player and all other players
    /// in the game that they should remove eachother from
    /// their player data list
    fn rem_user_sub(&self, target_id: PlayerID, target_link: SessionLink) {
        debug!("Removing user subscriptions");

        // Unsubscribe all the clients from eachother
        self.players
            .iter()
            .filter(|other| other.player.id.ne(&target_id))
            .for_each(|other| {
                let other_id = other.player.id;
                let other_link = other.link.clone();
                let target_link = target_link.clone();

                tokio::spawn(async move {
                    target_link.remove_subscriber(other_id).await;
                    other_link.remove_subscriber(target_id).await;
                });
            });
    }

    /// Notifies the provided player that the game has been setup and
    /// is ready for them to attempt to join.
    ///
    /// `session` The session to notify
    /// `slot`    The slot the player is joining into
    fn notify_game_setup(&self, player: &GamePlayer, context: GameSetupContext) {
        let packet = Packet::notify(
            game_manager::COMPONENT,
            game_manager::GAME_SETUP,
            GameSetupResponse {
                game: self,
                context,
            },
        );
        player.link.push(packet);
    }

    /// Modifies the psudo admin list this list doesn't actually exist in
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

        self.push_all(&Packet::notify(
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
        self.push_all(&packet);
        player.link.push(packet);
    }

    /// Attempts to migrate the host of this game if there are still players
    /// left in the game.
    fn try_migrate_host(&mut self) {
        // TODO: With more than one player this fails

        // Obtain the new player at the first index
        let (new_host_id, new_host_link) = match self.players.first() {
            Some(value) => (value.player.id, value.link.clone()),
            None => return,
        };

        debug!("Starting host migration (GID: {})", self.id);

        // Start host migration
        self.set_state(GameState::Migrating);
        self.push_all(&Packet::notify(
            game_manager::COMPONENT,
            game_manager::HOST_MIGRATION_START,
            HostMigrateStart {
                game_id: self.id,
                host_id: new_host_id,
                pmig: 2,
                slot: 0,
            },
        ));

        // Finished host migration
        self.set_state(GameState::InGame);
        self.push_all(&Packet::notify(
            game_manager::COMPONENT,
            game_manager::HOST_MIGRATION_FINISHED,
            HostMigrateFinished { game_id: self.id },
        ));

        self.add_user_sub(new_host_id, new_host_link);

        debug!("Finished host migration (GID: {})", self.id);
    }
}
