use crate::{
    database::entities::Player,
    services::{
        game::manager::RemoveGameMessage,
        matchmaking::{rules::RuleSet, CheckGameMessage},
    },
    session::{DetailsMessage, InformSessions, PushExt, Session, SetGameMessage},
    state::App,
    utils::{
        components::{Components, GameManager, UserSessions},
        models::NetData,
        types::{GameID, PlayerID},
    },
};
use blaze_pk::{
    codec::Encodable,
    packet::{Packet, PacketBody},
    types::TdfMap,
    writer::TdfWriter,
};
use interlink::prelude::*;
use log::debug;
use models::*;
use serde::Serialize;
use std::sync::Arc;

pub mod manager;
pub mod models;

/// Game service running within the server
pub struct Game {
    /// Unique ID for this game
    pub id: GameID,
    /// The current game state
    pub state: GameState,
    /// The current game setting
    pub setting: GameSettings,
    /// The game attributes
    pub attributes: AttrMap,
    /// The list of players in this game
    pub players: Vec<GamePlayer>,
}

impl Service for Game {
    fn stopping(&mut self) {
        debug!("Game is stopping (GID: {})", self.id);
        // Remove the stopping game
        let services = App::services();
        let _ = services
            .game_manager
            .do_send(RemoveGameMessage { game_id: self.id });
    }
}

impl Game {
    /// Starts a new game service with the provided initial state
    ///
    /// `id`         The unique ID for the game
    /// `attributes` The initial game attributes
    /// `setting`    The initial game setting value
    pub fn start(id: GameID, attributes: AttrMap, setting: GameSettings) -> Link<Game> {
        let this = Game {
            id,
            state: GameState::Initializing,
            setting,
            attributes,
            players: Vec::with_capacity(4),
        };

        this.start()
    }
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
    pub players: Vec<GamePlayerSnapshot>,
}

/// Attributes map type
pub type AttrMap = TdfMap<String, String>;

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
    pub display_name: String,
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
            display_name: self.player.display_name.clone(),
            net: if include_net {
                Some(self.net.clone())
            } else {
                None
            },
        }
    }

    pub fn encode(&self, game_id: GameID, slot: usize, writer: &mut TdfWriter) {
        writer.tag_empty_blob(b"BLOB");
        writer.tag_u8(b"EXID", 0);
        writer.tag_u32(b"GID", game_id);
        writer.tag_u32(b"LOC", 0x64654445);
        writer.tag_str(b"NAME", &self.player.display_name);
        writer.tag_u32(b"PID", self.player.id);
        self.net.tag_groups(b"PNET", writer);
        writer.tag_usize(b"SID", slot);
        writer.tag_u8(b"SLOT", 0);
        writer.tag_value(b"STAT", &self.state);
        writer.tag_u16(b"TIDX", 0xffff);
        writer.tag_u8(b"TIME", 0); /* Unix timestamp in millseconds */
        writer.tag_triple(b"UGID", (0, 0, 0));
        writer.tag_u32(b"UID", self.player.id);
        writer.tag_group_end();
    }
}

impl Drop for GamePlayer {
    fn drop(&mut self) {
        // Clear player game when game player is dropped
        self.set_game(None);
    }
}

/// Message to add a new player to this game
#[derive(Message)]
pub struct AddPlayerMessage {
    /// The player to add to the game
    pub player: GamePlayer,
    /// Context to which the player should be added
    pub context: GameSetupContext,
}

/// Handler for adding a player to the game
impl Handler<AddPlayerMessage> for Game {
    type Response = ();

    fn handle(&mut self, msg: AddPlayerMessage, _ctx: &mut ServiceContext<Self>) {
        let slot = self.players.len();

        self.players.push(msg.player);

        // Obtain the player that was just added
        let player = self
            .players
            .last()
            .expect("Player was added but is missing from players");

        // Whether the player was not the host player
        let is_other = slot != 0;

        if is_other {
            // Notify other players of the joined player
            self.notify_all(
                Components::GameManager(GameManager::PlayerJoining),
                PlayerJoining {
                    slot,
                    player,
                    game_id: self.id,
                },
            );

            // Update other players with the client details
            self.update_clients(player);
        }

        // Notify the joiner of the game details
        self.notify_game_setup(player, msg.context);

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
}

/// Message to alter the current game state
#[derive(Message)]
pub struct SetStateMessage {
    /// The new game state
    pub state: GameState,
}

/// Handler for setting the game state
impl Handler<SetStateMessage> for Game {
    type Response = ();

    fn handle(&mut self, msg: SetStateMessage, _ctx: &mut ServiceContext<Self>) {
        self.state = msg.state;
        self.notify_state();
    }
}

/// Message for setting the current game setting value
#[derive(Message)]
pub struct SetSettingMessage {
    /// The new setting value
    pub setting: GameSettings,
}

/// Handler for setting the game setting
impl Handler<SetSettingMessage> for Game {
    type Response = ();

    fn handle(&mut self, msg: SetSettingMessage, _ctx: &mut ServiceContext<Self>) {
        let setting = msg.setting;
        debug!("Updating game setting (Value: {:?})", &setting);
        self.setting = setting;
        self.notify_all(
            Components::GameManager(GameManager::GameSettingsChange),
            SettingChange {
                id: self.id,
                setting,
            },
        );
    }
}

/// Message for setting the game attributes
#[derive(Message)]
pub struct SetAttributesMessage {
    /// The new attributes
    pub attributes: AttrMap,
}

/// Handler for setting the game attributes
impl Handler<SetAttributesMessage> for Game {
    type Response = ();

    fn handle(&mut self, msg: SetAttributesMessage, ctx: &mut ServiceContext<Self>) {
        let attributes = msg.attributes;

        debug!("Updating game attributes");
        let packet = Packet::notify(
            Components::GameManager(GameManager::GameAttribChange),
            AttributesChange {
                id: self.id,
                attributes: &attributes,
            },
        );
        self.attributes.extend(attributes);
        self.push_all(&packet);

        // Don't update matchmaking for full games
        if self.players.len() < Self::MAX_PLAYERS {
            let services = App::services();
            let _ = services.matchmaking.do_send(CheckGameMessage {
                link: ctx.link(),
                game_id: self.id,
            });
        }
    }
}

/// Message to update the mesh connection state between
/// clients
#[derive(Message)]
pub struct UpdateMeshMessage {
    /// The ID of the session updating its connection
    pub id: PlayerID,
    /// The target player that its updating with
    pub target: PlayerID,
    /// The player mesh state
    pub state: PlayerState,
}

/// Handler for updating mesh connections
impl Handler<UpdateMeshMessage> for Game {
    type Response = ();

    fn handle(&mut self, msg: UpdateMeshMessage, _ctx: &mut ServiceContext<Self>) {
        let state = msg.state;
        if let PlayerState::ActiveConnecting = state {
            // Ensure the target player is in the game
            if !self
                .players
                .iter()
                .any(|value| value.player.id == msg.target)
            {
                return;
            }

            // Find the index of the session player
            let session = self
                .players
                .iter_mut()
                .find(|value| value.player.id == msg.id);

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

            // Notify players of the player state change
            self.notify_all(
                Components::GameManager(GameManager::GamePlayerStateChange),
                state_change,
            );

            // Notify players of the completed connection
            self.notify_all(
                Components::GameManager(GameManager::PlayerJoinCompleted),
                JoinComplete {
                    game_id: self.id,
                    player_id,
                },
            );

            // Add the player to the admin list
            self.modify_admin_list(player_id, AdminListOperation::Add);
        }
    }
}

/// Message for removing a player from the game
#[derive(Message)]
#[msg(rtype = "()")]
pub struct RemovePlayerMessage {
    /// The ID of the player/session to remove
    pub id: u32,
    /// The reason for removing the player
    pub reason: RemoveReason,
}

/// Handler for removing a player from the game
impl Handler<RemovePlayerMessage> for Game {
    type Response = ();
    fn handle(
        &mut self,
        msg: RemovePlayerMessage,
        ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        // Already empty game handling
        if self.players.is_empty() {
            ctx.stop();
            return;
        }

        // Find the player index
        let index = self.players.iter().position(|v| v.player.id == msg.id);

        let index = match index {
            Some(value) => value,
            None => return,
        };

        // Remove the player
        let player = self.players.remove(index);

        // Set current game of this player
        player.set_game(None);

        // Update the other players
        self.notify_player_removed(&player, msg.reason);
        self.notify_fetch_data(&player);
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
            ctx.stop();
        }
    }
}

/// Handler for checking if a game is joinable
#[derive(Message, Clone)]
#[msg(rtype = "GameJoinableState")]
pub struct CheckJoinableMessage {
    /// The player rule set if one is provided
    pub rule_set: Option<Arc<RuleSet>>,
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

/// Handler for checking if a game is joinable
impl Handler<CheckJoinableMessage> for Game {
    type Response = Mr<CheckJoinableMessage>;

    fn handle(
        &mut self,
        msg: CheckJoinableMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        // Handle full game
        if self.players.len() >= Self::MAX_PLAYERS {
            return Mr(GameJoinableState::Full);
        }

        // Check ruleset matches
        if let Some(rule_set) = msg.rule_set {
            if !rule_set.matches(&self.attributes) {
                return Mr(GameJoinableState::NotMatch);
            }
        }

        Mr(GameJoinableState::Joinable)
    }
}

/// Message to take a snapshot of the game and its state
#[derive(Message)]
#[msg(rtype = "GameSnapshot")]
pub struct SnapshotMessage {
    /// Whether to include the networking details in the snapshot
    pub include_net: bool,
}

/// Handler for taking snapshots of the game and its state
impl Handler<SnapshotMessage> for Game {
    type Response = Mr<SnapshotMessage>;

    fn handle(&mut self, msg: SnapshotMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        let players = self
            .players
            .iter()
            .map(|value| value.snapshot(msg.include_net))
            .collect();
        Mr(GameSnapshot {
            id: self.id,
            state: self.state,
            setting: self.setting.bits(),
            attributes: self.attributes.clone(),
            players,
        })
    }
}

/// Message for getting an encoded packet body of the game data
#[derive(Message)]
#[msg(rtype = "PacketBody")]
pub struct GetGameDataMessage;

/// Handler for getting an encoded packet body of the game data
impl Handler<GetGameDataMessage> for Game {
    type Response = Mr<GetGameDataMessage>;

    fn handle(
        &mut self,
        _msg: GetGameDataMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        let data = GetGameDetails { game: self };
        let data: PacketBody = data.into();
        Mr(data)
    }
}

impl Game {
    /// Constant for the maximum number of players allowed in
    /// a game at one time. Used to determine a games full state
    const MAX_PLAYERS: usize = 4;

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

    /// Sends a notification packet to all the connected session
    /// with the provided component and contents
    ///
    /// `component` The packet component
    /// `contents`  The packet contents
    fn notify_all<C: Encodable>(&self, component: Components, contents: C) {
        let packet = Packet::notify(component, contents);
        self.push_all(&packet);
    }

    /// Notifies all players of the current game state
    fn notify_state(&self) {
        self.notify_all(
            Components::GameManager(GameManager::GameStateChange),
            StateChange {
                id: self.id,
                state: self.state,
            },
        );
    }

    /// Updates all the client details for the provided session.
    /// Tells each client to send session updates to the session
    /// and the session to send them as well.
    ///
    /// `session` The session to update for
    fn update_clients(&self, player: &GamePlayer) {
        debug!("Updating clients with new session details");
        self.players.iter().for_each(|value| {
            if value.player.id != player.player.id {
                let addr1 = player.link.clone();
                let addr2 = value.link.clone();

                // Queue the session details to be sent to this client
                let _ = player.link.do_send(DetailsMessage { link: addr2 });
                let _ = value.link.do_send(DetailsMessage { link: addr1 });
            }
        });
    }

    /// Notifies the provided player that the game has been setup and
    /// is ready for them to attempt to join.
    ///
    /// `session` The session to notify
    /// `slot`    The slot the player is joining into
    fn notify_game_setup(&self, player: &GamePlayer, context: GameSetupContext) {
        let packet = Packet::notify(
            Components::GameManager(GameManager::GameSetup),
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
    fn modify_admin_list(&self, target: PlayerID, operation: AdminListOperation) {
        let host = match self.players.first() {
            Some(value) => value,
            None => return,
        };

        self.notify_all(
            Components::GameManager(GameManager::AdminListChange),
            AdminListChange {
                game_id: self.id,
                player_id: target,
                operation,
                host_id: host.player.id,
            },
        );
    }

    /// Notifies all the session and the removed session that a
    /// session was removed from the game.
    ///
    /// `player`    The player that was removed
    /// `player_id` The player ID of the removed player
    fn notify_player_removed(&self, player: &GamePlayer, reason: RemoveReason) {
        let packet = Packet::notify(
            Components::GameManager(GameManager::PlayerRemoved),
            PlayerRemoved {
                game_id: self.id,
                player_id: player.player.id,
                reason,
            },
        );
        self.push_all(&packet);
        player.link.push(packet);
    }

    /// Notifies all the sessions in the game to fetch the player data
    /// for the provided session and the session to fetch the extended
    /// data for all the other sessions. Will early return if there
    /// are no players left.
    ///
    /// `session`   The session to update with the other clients
    /// `player_id` The player id of the session to update
    fn notify_fetch_data(&self, player: &GamePlayer) {
        self.notify_all(
            Components::UserSessions(UserSessions::FetchExtendedData),
            FetchExtendedData {
                player_id: player.player.id,
            },
        );

        for other_player in &self.players {
            let packet = Packet::notify(
                Components::UserSessions(UserSessions::FetchExtendedData),
                FetchExtendedData {
                    player_id: other_player.player.id,
                },
            );
            player.link.push(packet)
        }
    }

    /// Attempts to migrate the host of this game if there are still players
    /// left in the game.
    fn try_migrate_host(&mut self) {
        // TODO: With more than one player this fails

        // Obtain the new player at the first index
        let new_host = match self.players.first() {
            Some(value) => value,
            None => return,
        };

        debug!("Starting host migration (GID: {})", self.id);

        // Start host migration
        self.state = GameState::Migrating;
        self.notify_state();
        self.notify_all(
            Components::GameManager(GameManager::HostMigrationStart),
            HostMigrateStart {
                game_id: self.id,
                host_id: new_host.player.id,
            },
        );

        // Finished host migration
        self.state = GameState::InGame;
        self.notify_state();
        self.notify_all(
            Components::GameManager(GameManager::HostMigrationFinished),
            HostMigrateFinished { game_id: self.id },
        );

        self.update_clients(new_host);

        debug!("Finished host migration (GID: {})", self.id);
    }
}
