use self::{manager::GameManager, rules::RuleSet};
use crate::{
    database::entities::Player,
    session::{
        packet::Packet, router::RawBlaze, DetailsMessage, InformSessions, PushExt, Session,
        SetGameMessage,
    },
    utils::{
        components::{game_manager, user_sessions},
        models::NetData,
        types::{GameID, PlayerID},
    },
};
use interlink::prelude::*;
use log::debug;
use models::*;
use serde::Serialize;
use std::sync::Arc;
use tdf::{ObjectId, TdfMap, TdfSerializer};
use tokio::sync::RwLock;

pub mod manager;
pub mod models;
pub mod rules;

pub type GameRef = Arc<RwLock<Game>>;

pub struct Game {
    id: GameID,
    state: GameState,
    settings: GameSettings,
    attributes: Attributes,
    players: Vec<GamePlayer>,
    game_manager: Arc<GameManager>,
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
    pub attributes: Attributes,
    /// Snapshots of the game players
    pub players: Box<[GamePlayerSnapshot]>,
}

/// Attributes map type
pub type Attributes = TdfMap<String, String>;

/// Player structure containing details and state for a player
/// within a game
pub struct GamePlayer {
    /// Session player
    pub player: Player,
    /// Session address
    pub link: Link<Session>,
    /// Networking information for the player
    pub net: NetData,
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
    pub net: Option<NetData>,
}

impl GamePlayer {
    /// Creates a new game player structure with the provided player
    /// details
    ///
    /// `player` The session player
    /// `net`    The player networking details
    /// `addr`   The session address
    pub fn new(player: Player, net: NetData, link: Link<Session>) -> Self {
        Self {
            player,
            link,
            net,
            state: PlayerState::ActiveConnecting,
        }
    }

    pub fn set_game(&self, game: Option<GameID>) {
        let _ = self.link.do_send(SetGameMessage { game });
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
        w.tag_group_end();
    }
}

impl Drop for GamePlayer {
    fn drop(&mut self) {
        // Clear player game when game player is dropped
        self.set_game(None);
    }
}

impl Game {
    const MAX_PLAYERS: usize = 4;

    pub fn new(
        id: GameID,
        settings: GameSettings,
        attributes: Attributes,
        game_manager: Arc<GameManager>,
    ) -> Self {
        Self {
            id,
            state: Default::default(),
            players: Default::default(),
            settings,
            attributes,
            game_manager,
        }
    }

    pub async fn stop(&self) {
        // TODO: Remove players from game
        self.game_manager.remove_game(self.id).await;
    }

    /// ONLY CALL FROM GAME MANAGER WHEN DESTRUCTED
    pub async fn on_stopped(&mut self) {
        debug!("Stopped game (GID: {})", self.id);
    }

    pub async fn set_state(&mut self, state: GameState) {
        self.state = state;
        self.push_all(&Packet::notify(
            game_manager::COMPONENT,
            game_manager::GAME_STATE_CHANGE,
            StateChange { id: self.id, state },
        ))
        .await;
    }

    pub async fn set_settings(&mut self, settings: GameSettings) {
        self.settings = settings;
        self.push_all(&Packet::notify(
            game_manager::COMPONENT,
            game_manager::GAME_SETTINGS_CHANGE,
            SettingsChange {
                id: self.id,
                settings,
            },
        ))
        .await;
    }

    pub async fn check_joinable(&self, rule_set: Option<Arc<RuleSet>>) -> GameJoinableState {
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

    pub async fn set_attributes(&mut self, attributes: Attributes) {
        // Packet is created before attributes are consumed to prevent extra cloning
        let update_packet = Packet::notify(
            game_manager::COMPONENT,
            game_manager::GAME_ATTRIB_CHANGE,
            AttributesChange {
                id: self.id,
                attributes: &attributes,
            },
        );

        self.attributes.insert_presorted(attributes.into_inner());

        self.push_all(&update_packet).await;
        self.update_matchmaking().await;
    }

    /// Updates the matchmaking process to handle attempting
    /// to join players into this game if possible
    pub async fn update_matchmaking(&self) {
        if self.players.len() >= Self::MAX_PLAYERS {
            return;
        }

        let game_manager = self.game_manager.clone();
        let game_id = self.id;
        tokio::spawn(async move {
            game_manager.process_queue(game_id).await;
        });
    }

    pub async fn push_all(&self, packet: &Packet) {
        self.players
            .iter()
            .for_each(|player| player.link.push(packet.clone()))
    }

    pub async fn snapshot(&self, include_net: bool) -> GameSnapshot {
        let players = self
            .players
            .iter()
            .map(|player| player.snapshot(include_net))
            .collect();

        GameSnapshot {
            id: self.id,
            state: self.state,
            setting: self.settings.bits(),
            attributes: self.attributes.clone(),
            players,
        }
    }

    pub async fn add_player(&mut self, player: GamePlayer, context: GameSetupContext) {
        let slot = self.players.len();
        self.players.push(player);

        // Obtain the player that was just added
        let player = self
            .players
            .last()
            .expect("Player was added but is missing from players");

        // Whether the player was not the host player
        let is_other = slot != 0;

        if is_other {
            // Notify other players of the joined player
            self.push_all(&Packet::notify(
                game_manager::COMPONENT,
                game_manager::PLAYER_JOINING,
                PlayerJoining {
                    slot,
                    player,
                    game_id: self.id,
                },
            ))
            .await;

            // Update other players with the client details
            self.update_clients(player.player.id, player.link.clone())
                .await;
        }

        // Notify the joiner of the game details
        self.notify_game_setup(player, context).await;

        // Set current game of this player
        player.set_game(Some(self.id));

        if is_other {
            // Provide the new players session details to the other players
            let links: Vec<Link<Session>> = self
                .players
                .iter()
                .map(|player| player.link.clone())
                .collect();
            let _ = player.link.do_send(InformSessions { links });
        }
    }

    pub async fn update_mesh(&mut self, id: PlayerID, target: PlayerID, state: PlayerState) {
        if let PlayerState::ActiveConnecting = state {
            let player_id = {
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
                session.player.id
            };

            let state_change = PlayerStateChange {
                gid: self.id,
                pid: player_id,
                state: PlayerState::ActiveConnected,
            };

            // Notify players of the player state change
            self.push_all(&Packet::notify(
                game_manager::COMPONENT,
                game_manager::GAME_PLAYER_STATE_CHANGE,
                state_change,
            ))
            .await;

            // Notify players of the completed connection
            self.push_all(&Packet::notify(
                game_manager::COMPONENT,
                game_manager::PLAYER_JOIN_COMPLETED,
                JoinComplete {
                    game_id: self.id,
                    player_id,
                },
            ))
            .await;

            // Add the player to the admin list
            self.modify_admin_list(player_id, AdminListOperation::Add)
                .await;
        }
    }

    pub async fn remove_player(&mut self, id: u32, reason: RemoveReason) {
        // Already empty game handling
        if self.players.is_empty() {
            self.stop().await;
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
        self.notify_player_removed(&player, reason).await;
        self.notify_fetch_data(&player).await;
        self.modify_admin_list(player.player.id, AdminListOperation::Remove)
            .await;

        debug!(
            "Removed player from game (PID: {}, GID: {})",
            player.player.id, self.id
        );

        // If the player was in the host slot attempt migration
        if index == 0 {
            self.try_migrate_host().await;
        }

        if self.players.is_empty() {
            // Game is empty stop it
            self.stop().await;
        }
    }

    pub async fn game_data(&self) -> RawBlaze {
        let data = GetGameDetails { game: self };
        RawBlaze::from(data)
    }

    /// Updates all the client details for the provided session.
    /// Tells each client to send session updates to the session
    /// and the session to send them as well.
    ///
    /// `session` The session to update for
    async fn update_clients(&self, target_id: PlayerID, target_link: Link<Session>) {
        debug!("Updating clients with new session details");
        self.players.iter().for_each(|value| {
            if value.player.id != target_id {
                let addr1 = target_link.clone();
                let addr2 = value.link.clone();

                // Queue the session details to be sent to this client
                let _ = target_link.do_send(DetailsMessage { link: addr2 });
                let _ = value.link.do_send(DetailsMessage { link: addr1 });
            }
        });
    }

    /// Notifies the provided player that the game has been setup and
    /// is ready for them to attempt to join.
    ///
    /// `session` The session to notify
    /// `slot`    The slot the player is joining into
    async fn notify_game_setup(&self, player: &GamePlayer, context: GameSetupContext) {
        let packet = Packet::notify(
            game_manager::COMPONENT,
            game_manager::GAME_SETUP,
            GameDetails {
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
    async fn modify_admin_list(&self, target: PlayerID, operation: AdminListOperation) {
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
        ))
        .await;
    }

    /// Notifies all the session and the removed session that a
    /// session was removed from the game.
    ///
    /// `player`    The player that was removed
    /// `player_id` The player ID of the removed player
    async fn notify_player_removed(&self, player: &GamePlayer, reason: RemoveReason) {
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
        self.push_all(&packet).await;
        player.link.push(packet);
    }

    /// Notifies all the sessions in the game to fetch the player data
    /// for the provided session and the session to fetch the extended
    /// data for all the other sessions. Will early return if there
    /// are no players left.
    ///
    /// `session`   The session to update with the other clients
    /// `player_id` The player id of the session to update
    async fn notify_fetch_data(&self, player: &GamePlayer) {
        self.push_all(&Packet::notify(
            user_sessions::COMPONENT,
            user_sessions::FETCH_EXTENDED_DATA,
            FetchExtendedData {
                player_id: player.player.id,
            },
        ))
        .await;

        for other_player in &self.players {
            let packet = Packet::notify(
                user_sessions::COMPONENT,
                user_sessions::FETCH_EXTENDED_DATA,
                FetchExtendedData {
                    player_id: other_player.player.id,
                },
            );
            player.link.push(packet)
        }
    }

    /// Attempts to migrate the host of this game if there are still players
    /// left in the game.
    async fn try_migrate_host(&mut self) {
        // TODO: With more than one player this fails

        // Obtain the new player at the first index
        let (new_host_id, new_host_link) = match self.players.first() {
            Some(value) => (value.player.id, value.link.clone()),
            None => return,
        };

        debug!("Starting host migration (GID: {})", self.id);

        // Start host migration
        self.set_state(GameState::Migrating).await;

        self.push_all(&Packet::notify(
            game_manager::COMPONENT,
            game_manager::HOST_MIGRATION_START,
            HostMigrateStart {
                game_id: self.id,
                host_id: new_host_id,
                pmig: 2,
                slot: 0,
            },
        ))
        .await;

        // Finished host migration

        self.set_state(GameState::InGame).await;

        self.push_all(&Packet::notify(
            game_manager::COMPONENT,
            game_manager::HOST_MIGRATION_FINISHED,
            HostMigrateFinished { game_id: self.id },
        ))
        .await;

        self.update_clients(new_host_id, new_host_link).await;

        debug!("Finished host migration (GID: {})", self.id);
    }
}
