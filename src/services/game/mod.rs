use super::matchmaking::rules::RuleSet;
use crate::{
    servers::main::session::{DetailsMessage, InformSessions, PushExt, Session},
    utils::{
        components::{Components, GameManager, UserSessions},
        types::{GameID, GameSlot, PlayerID, SessionID},
    },
};
use blaze_pk::{
    codec::Encodable,
    packet::{Packet, PacketBody},
    types::TdfMap,
};
use interlink::prelude::*;
use log::debug;
use models::*;
use player::{GamePlayer, GamePlayerSnapshot};
use serde::Serialize;
use std::sync::Arc;

pub mod manager;
pub mod models;
pub mod player;

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
}

impl Service for Game {
    fn stopping(&mut self) {
        debug!("Game is stopping (GID: {})", self.id)
    }
}

impl Game {
    pub fn start(id: GameID, attributes: AttrMap, setting: u16) -> Link<Game> {
        let this = Game {
            id,
            state: GameState::Init,
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
    pub id: GameID,
    pub state: GameState,
    pub setting: u16,
    pub attributes: AttrMap,
    pub players: Vec<GamePlayerSnapshot>,
}

/// Attributes map type
pub type AttrMap = TdfMap<String, String>;
/// Message to add a new player to this game
#[derive(Message)]
pub struct AddPlayerMessage {
    /// The player to add to the game
    pub player: GamePlayer,
}

impl Handler<AddPlayerMessage> for Game {
    type Response = ();

    fn handle(&mut self, msg: AddPlayerMessage, _ctx: &mut ServiceContext<Self>) {
        let slot = self.players.len();

        self.players.push(msg.player);

        // Obtain the player that was just added
        let player = match self.players.last() {
            Some(value) => value,
            None => return,
        };

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
        self.notify_game_setup(player, slot);

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
    pub setting: u16,
}

impl Handler<SetSettingMessage> for Game {
    type Response = ();

    fn handle(&mut self, msg: SetSettingMessage, _ctx: &mut ServiceContext<Self>) {
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
#[derive(Message)]
pub struct SetAttributesMessage {
    /// The new attributes
    pub attributes: AttrMap,
}

impl Handler<SetAttributesMessage> for Game {
    type Response = ();

    fn handle(&mut self, msg: SetAttributesMessage, _ctx: &mut ServiceContext<Self>) {
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
#[derive(Message)]
pub struct UpdateMeshMessage {
    /// The ID of the session updating its connection
    pub session: SessionID,
    /// The target player that its updating with
    pub target: PlayerID,
    /// The mesh player state
    pub state: PlayerState,
}

impl Handler<UpdateMeshMessage> for Game {
    type Response = ();

    fn handle(&mut self, msg: UpdateMeshMessage, _ctx: &mut ServiceContext<Self>) {
        let state = msg.state;
        if let PlayerState::Connected = state {
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
                .find(|value| value.session_id == msg.session);

            let session = match session {
                Some(value) => value,
                None => return,
            };

            // Update the session state
            session.state = PlayerState::Connected;

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

#[derive(Message)]
#[msg(rtype = "bool")]
pub struct RemovePlayerMessage {
    pub id: u32,
    pub reason: RemoveReason,
    pub ty: RemovePlayerType,
}

#[derive(Debug)]
pub enum RemovePlayerType {
    Session,
    Player,
}

impl Handler<RemovePlayerMessage> for Game {
    type Response = Mr<RemovePlayerMessage>;
    fn handle(
        &mut self,
        msg: RemovePlayerMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        // Already empty game handling
        if self.players.is_empty() {
            return Mr(true);
        }

        // Find the player index
        let index = match msg.ty {
            RemovePlayerType::Player => self.players.iter().position(|v| v.player.id == msg.id),
            RemovePlayerType::Session => self.players.iter().position(|v| v.session_id == msg.id),
        };

        let index = match index {
            Some(value) => value,
            None => return Mr(false),
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

        Mr(self.players.is_empty())
    }
}

#[derive(Message)]
#[msg(rtype = "GameJoinableState")]
pub struct CheckJoinableMessage {
    pub rule_set: Option<Arc<RuleSet>>,
}

impl Handler<CheckJoinableMessage> for Game {
    type Response = Mr<CheckJoinableMessage>;

    fn handle(
        &mut self,
        msg: CheckJoinableMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        let is_joinable = self.players.len() < Self::MAX_PLAYERS;
        if let Some(rule_set) = msg.rule_set {
            if !rule_set.matches(&self.attributes) {
                return Mr(GameJoinableState::NotMatch);
            }
        }

        Mr(if is_joinable {
            GameJoinableState::Joinable
        } else {
            GameJoinableState::Full
        })
    }
}

#[derive(Message)]
#[msg(rtype = "GameSnapshot")]
pub struct SnapshotMessage {
    pub include_net: bool,
}

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
            setting: self.setting,
            attributes: self.attributes.clone(),
            players,
        })
    }
}

#[derive(Message)]
#[msg(rtype = "PacketBody")]
pub struct GetGameDataMessage;

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
            if value.session_id != player.session_id {
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
    fn notify_game_setup(&self, player: &GamePlayer, slot: GameSlot) {
        let ty = if slot == 0 {
            GameDetailsType::Created
        } else {
            GameDetailsType::Joined(player.session_id)
        };
        let packet = Packet::notify(
            Components::GameManager(GameManager::GameSetup),
            GameDetails { game: self, ty },
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
        // Obtain the new player at the first index
        let new_host = match self.players.first() {
            Some(value) => value,
            None => return,
        };

        debug!("Starting host migration (GID: {})", self.id);

        // Start host migration
        self.state = GameState::HostMigration;
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
