use self::rules::RuleSet;
use crate::utils::{
    components::{Components, GameManager, UserSessions},
    types::{GameID, GameSlot, PlayerID, SessionID},
};
use blaze_pk::{codec::Encodable, packet::Packet, types::TdfMap};
use interlink::{msg::MessageResponse, prelude::*};
use log::debug;
use models::*;
use player::{GamePlayer, GamePlayerSnapshot};
use serde::Serialize;
use std::sync::Arc;

pub mod manager;
pub mod matchmaking;
pub mod models;
pub mod player;
pub mod rules;

pub struct Game {
    /// Unique ID for this game
    pub id: GameID,
    /// The current game state
    pub state: GameState,
    /// The current game setting
    pub setting: u16,
    /// The game attributes
    pub attributes: AttrMap,
    /// The list of players in this game
    pub players: Vec<GamePlayer>,
    /// The number of the next available slot
    pub next_slot: GameSlot,
}

impl Service for Game {}

impl Game {
    pub fn start(id: GameID, attributes: AttrMap, setting: u16) -> Link<Game> {
        let this = Game {
            id,
            state: GameState::Init,
            setting,
            attributes,
            players: Vec::new(),
            next_slot: 0,
        };
        this.start()
    }
}

#[derive(Serialize)]
pub struct GameSnapshot {
    pub id: GameID,
    pub state: GameState,
    pub setting: u16,
    pub attributes: AttrMap,
    pub players: Vec<GamePlayerSnapshot>,
}

/// Attributes map type
pub type AttrMap = TdfMap<String, String>;
/// Message to add a new player to this game
pub struct AddPlayerMessage {
    /// The player to add to the game
    pub player: GamePlayer,
}

impl Message for AddPlayerMessage {
    type Response = ();
}

impl Handler<AddPlayerMessage> for Game {
    type Response = ();

    fn handle(&mut self, msg: AddPlayerMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        let mut player = msg.player;
        let slot = self.aquire_slot();
        player.game_id = self.id;

        self.notify_player_joining(&player, slot);
        self.update_clients(&player);
        self.notify_game_setup(&player, slot);

        let id = self.id;

        player
            .addr
            .link
            .do_exec(move |session, _| session.set_game(Some(id)))
            .ok();

        let packet = player.create_set_session();
        self.push_all(&packet);

        self.players.push(player);

        debug!("Adding player complete");
    }
}

/// Message to alter the current game state
pub struct SetStateMessage {
    /// The new game state
    pub state: GameState,
}

impl Message for SetStateMessage {
    type Response = ();
}

impl Handler<SetStateMessage> for Game {
    type Response = ();

    fn handle(&mut self, msg: SetStateMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        self.set_state(msg.state)
    }
}

/// Message for setting the current game setting value
pub struct SetSettingMessage {
    /// The new setting value
    pub setting: u16,
}

impl Message for SetSettingMessage {
    type Response = ();
}

impl Handler<SetSettingMessage> for Game {
    type Response = ();

    fn handle(
        &mut self,
        msg: SetSettingMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        let setting = msg.setting;
        debug!("Updating game setting (Value: {})", &setting);
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
pub struct SetAttributesMessage {
    /// The new attributes
    pub attributes: AttrMap,
}

impl Message for SetAttributesMessage {
    type Response = ();
}

impl Handler<SetAttributesMessage> for Game {
    type Response = ();

    fn handle(
        &mut self,
        msg: SetAttributesMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
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
    }
}

/// Message to update the mesh connection state between
/// clients
pub struct UpdateMeshMessage {
    /// The ID of the session updating its connection
    pub session: SessionID,
    /// The target player that its updating with
    pub target: PlayerID,
    /// The mesh player state
    pub state: PlayerState,
}

impl Message for UpdateMeshMessage {
    type Response = ();
}

impl Handler<UpdateMeshMessage> for Game {
    type Response = ();

    fn handle(
        &mut self,
        msg: UpdateMeshMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        let state = msg.state;
        let session = msg.session;
        debug!("Updating mesh connection");
        match state {
            PlayerState::Disconnected => {
                debug!("Disconnected mesh")
            }
            PlayerState::Connecting => {
                if self.is_player_sid(session) && self.is_player_pid(msg.target) {
                    self.set_player_state(session, PlayerState::Connected);
                    self.on_join_complete(session);
                    debug!("Connected player to game")
                } else {
                    debug!("Connected player mesh")
                }
            }
            PlayerState::Connected => {}
            _ => {}
        }
    }
}

pub struct RemovePlayerMessage {
    pub ty: RemovePlayerType,
}

impl Message for RemovePlayerMessage {
    type Response = bool;
}

impl Handler<RemovePlayerMessage> for Game {
    type Response = MessageResponse<RemovePlayerMessage>;
    fn handle(
        &mut self,
        msg: RemovePlayerMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        MessageResponse(self.remove_player(msg.ty))
    }
}

pub struct CheckJoinableMessage {
    pub rule_set: Arc<RuleSet>,
}

impl Message for CheckJoinableMessage {
    type Response = GameJoinableState;
}

impl Handler<CheckJoinableMessage> for Game {
    type Response = MessageResponse<CheckJoinableMessage>;

    fn handle(
        &mut self,
        msg: CheckJoinableMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        let is_joinable = self.next_slot < Self::MAX_PLAYERS;
        if !msg.rule_set.matches(&self.attributes) {
            return MessageResponse(GameJoinableState::NotMatch);
        }

        MessageResponse(if is_joinable {
            GameJoinableState::Joinable
        } else {
            GameJoinableState::Full
        })
    }
}

pub struct SnapshotMessage;

impl Message for SnapshotMessage {
    type Response = GameSnapshot;
}

impl Handler<SnapshotMessage> for Game {
    type Response = MessageResponse<SnapshotMessage>;
    fn handle(&mut self, _msg: SnapshotMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        let players = self.players.iter().map(|value| value.snapshot()).collect();
        MessageResponse(GameSnapshot {
            id: self.id,
            state: self.state,
            setting: self.setting,
            attributes: self.attributes.clone(),
            players,
        })
    }
}

pub enum GameJoinableState {
    /// Game is currenlty joinable
    Joinable,
    /// Game is full
    Full,
    /// The game doesn't match the provided rules
    NotMatch,
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
            .for_each(|value| value.addr.push(packet.clone()));
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

    /// Sets the current game state in the game data and
    /// sends an update notification to all connected clients
    /// notifying them of the changed state
    ///
    /// `state` The new state value
    fn set_state(&mut self, state: GameState) {
        debug!("Updating game state (Value: {state:?})");
        self.state = state;
        self.notify_all(
            Components::GameManager(GameManager::GameStateChange),
            StateChange { id: self.id, state },
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
            let addr1 = player.addr.clone();
            let addr2 = value.addr.clone();

            value
                .addr
                .link
                .do_exec(|session, _| session.push_details(addr1))
                .ok();
            player
                .addr
                .link
                .do_exec(|session, _| session.push_details(addr2))
                .ok();
        });
    }

    /// Checks whether the provided session is a player in this game
    ///
    /// `session` The session to check for
    fn is_player_sid(&self, sid: SessionID) -> bool {
        self.players.iter().any(|value| value.session_id == sid)
    }

    /// Checks whether this game contains a player with the provided
    /// player ID
    ///
    /// `pid` The player ID
    fn is_player_pid(&self, pid: PlayerID) -> bool {
        self.players.iter().any(|value| value.session_id == pid)
    }

    fn aquire_slot(&mut self) -> usize {
        let slot = self.next_slot;
        self.next_slot += 1;
        slot
    }

    fn release_slot(&mut self) {
        self.next_slot -= 1;
    }

    /// Notifies all the players in the game that a new player has
    /// joined the game.
    fn notify_player_joining(&self, player: &GamePlayer, slot: GameSlot) {
        if slot == 0 {
            return;
        }
        let packet = Packet::notify(
            Components::GameManager(GameManager::PlayerJoining),
            PlayerJoining { slot, player },
        );
        self.push_all(&packet);
        player.addr.push(packet);
    }

    /// Notifies the provided player that the game has been setup and
    /// is ready for them to attempt to join.
    ///
    /// `session` The session to notify
    /// `slot`    The slot the player is joining into
    fn notify_game_setup(&self, player: &GamePlayer, slot: GameSlot) {
        let ty = match slot {
            0 => GameDetailsType::Created,
            _ => GameDetailsType::Joined,
        };

        let packet = Packet::notify(
            Components::GameManager(GameManager::GameSetup),
            GameDetails {
                game: self,
                player,
                ty,
            },
        );

        player.addr.push(packet);
    }

    /// Sets the state for the provided session notifying all
    /// the players that the players state has changed.
    ///
    /// `session` The session to change the state of
    /// `state`   The new state value
    fn set_player_state(&mut self, session: SessionID, state: PlayerState) -> Option<PlayerState> {
        let (player_id, old_state) = {
            let player = self
                .players
                .iter_mut()
                .find(|value| value.session_id == session)?;
            let old_state = player.state;
            player.state = state;
            (player.player.id, old_state)
        };

        let packet = Packet::notify(
            Components::GameManager(GameManager::GamePlayerStateChange),
            PlayerStateChange {
                gid: self.id,
                pid: player_id,
                state,
            },
        );
        self.push_all(&packet);
        Some(old_state)
    }

    /// Modifies the psudo admin list this list doesn't actually exist in
    /// our implementation but we still need to tell the clients these
    /// changes.
    ///
    /// `target`    The player to target for the admin list
    /// `operation` Whether to add or remove the player from the admin list
    fn modify_admin_list(&self, target: PlayerID, operation: AdminListOperation) {
        let host_id = {
            let Some(host) = self.players.first() else {
                return;
            };
            host.player.id
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
        self.push_all(&packet);
    }

    /// Handles informing the other players in the game when a player joining
    /// is complete (After the mesh connection is updated) and modifies the
    /// admin list to include the newly added session
    ///
    /// `session` The session that completed joining
    fn on_join_complete(&self, session: SessionID) {
        let Some(player) = self.players.iter().find(|value| value.session_id == session) else {
            return;
        };
        let packet = Packet::notify(
            Components::GameManager(GameManager::PlayerJoinCompleted),
            JoinComplete {
                game_id: self.id,
                player_id: player.player.id,
            },
        );
        self.push_all(&packet);
        self.modify_admin_list(player.player.id, AdminListOperation::Add);
    }

    fn remove_player(&mut self, ty: RemovePlayerType) -> bool {
        let (player, slot, reason, is_empty) = {
            if self.players.is_empty() {
                return true;
            }
            let (index, reason) = match ty {
                RemovePlayerType::Player(player_id, reason) => (
                    self.players
                        .iter()
                        .position(|value| value.player.id == player_id),
                    reason,
                ),
                RemovePlayerType::Session(session_id) => (
                    self.players
                        .iter()
                        .position(|value| value.session_id == session_id),
                    RemoveReason::Generic,
                ),
            };

            let (player, index) = match index {
                Some(index) => (self.players.remove(index), index),
                None => return false,
            };
            (player, index, reason, self.players.is_empty())
        };

        player
            .addr
            .link
            .do_exec(|session, _| session.set_game(None))
            .ok();

        self.notify_player_removed(&player, reason);
        self.notify_fetch_data(&player);
        self.modify_admin_list(player.player.id, AdminListOperation::Remove);

        // Possibly not needed
        // let packet = player.create_set_session();
        // self.push_all(&packet);
        debug!(
            "Removed player from game (PID: {}, GID: {})",
            player.player.id, self.id
        );
        // If the player was in the host slot
        if slot == 0 {
            self.try_migrate_host();
        }
        self.release_slot();

        is_empty
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
        player.addr.push(packet);
    }

    /// Notifies all the sessions in the game to fetch the player data
    /// for the provided session and the session to fetch the extended
    /// data for all the other sessions. Will early return if there
    /// are no players left.
    ///
    /// `session`   The session to update with the other clients
    /// `player_id` The player id of the session to update
    fn notify_fetch_data(&self, player: &GamePlayer) {
        let removed_packet = Packet::notify(
            Components::UserSessions(UserSessions::FetchExtendedData),
            FetchExtendedData {
                player_id: player.player.id,
            },
        );
        self.push_all(&removed_packet);

        for other_player in &self.players {
            let packet = Packet::notify(
                Components::UserSessions(UserSessions::FetchExtendedData),
                FetchExtendedData {
                    player_id: other_player.player.id,
                },
            );
            player.addr.push(packet)
        }
    }

    /// Attempts to migrate the host of this game if there are still players
    /// left in the game.
    fn try_migrate_host(&mut self) {
        self.set_state(GameState::HostMigration);
        debug!("Starting host migration (GID: {})", self.id);
        self.notify_migrate_start();
        self.set_state(GameState::InGame);
        self.notify_migrate_finish();
        let Some(new_host) = self.players.first() else { return; };
        self.update_clients(new_host);

        debug!("Finished host migration (GID: {})", self.id);
    }

    /// Notifies all the sessions in this game that host migration has
    /// begun.
    ///
    /// `new_host` The session that is being migrated to host
    fn notify_migrate_start(&self) {
        let Some(new_host) = self.players.first() else { return; };
        let packet = Packet::notify(
            Components::GameManager(GameManager::HostMigrationStart),
            HostMigrateStart {
                game_id: self.id,
                host_id: new_host.player.id,
            },
        );
        self.push_all(&packet);
    }

    /// Notifies to all sessions that the migration is complete
    fn notify_migrate_finish(&self) {
        let packet = Packet::notify(
            Components::GameManager(GameManager::HostMigrationFinished),
            HostMigrateFinished { game_id: self.id },
        );
        self.push_all(&packet);
    }
}

impl Drop for Game {
    fn drop(&mut self) {
        debug!("Game has been dropped (GID: {})", self.id)
    }
}

#[derive(Debug)]
pub enum RemovePlayerType {
    Session(SessionID),
    Player(PlayerID, RemoveReason),
}
