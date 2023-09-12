use self::{manager::GameManager, rules::RuleSet};
use crate::{
    database::entities::Player,
    session::{
        models::game_manager::{
            AdminListChange, AdminListOperation, AttributesChange, GameSettings, GameState,
            HostMigrateFinished, HostMigrateStart, JoinComplete, PlayerJoining, PlayerRemoved,
            PlayerState, PlayerStateChange, RemoveReason, SettingChange, StateChange,
        },
        packet::Packet,
        router::RawBlaze,
        NetData, PushExt, Session, SessionLink, SetGameMessage, SubscriberMessage,
    },
    utils::{
        components::game_manager,
        types::{GameID, PlayerID},
    },
};
use interlink::prelude::*;
use log::debug;
use models::*;
use serde::Serialize;
use std::sync::Arc;
use tdf::{ObjectId, TdfMap, TdfSerializer};

pub mod manager;
pub mod models;
pub mod rules;

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
    /// Services access
    pub game_manager: Arc<GameManager>,
}

impl Service for Game {
    fn stopping(&mut self) {
        debug!("Game is stopping (GID: {})", self.id);

        // Remove the stopping game
        let game_manager = self.game_manager.clone();
        let game_id = self.id;
        tokio::spawn(async move {
            game_manager.remove_game(game_id).await;
        });
    }
}

impl Game {
    /// Starts a new game service with the provided initial state
    ///
    /// `id`         The unique ID for the game
    /// `attributes` The initial game attributes
    /// `setting`    The initial game setting value
    pub fn start(
        id: GameID,
        attributes: AttrMap,
        setting: GameSettings,
        game_manager: Arc<GameManager>,
    ) -> Link<Game> {
        let this = Game {
            id,
            state: Default::default(),
            setting,
            attributes,
            players: Default::default(),
            game_manager,
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
    pub link: Link<Session>,
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
    pub fn new(player: Arc<Player>, net: Arc<NetData>, link: Link<Session>) -> Self {
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
        self.notify_game_setup(player, msg.context);

        // Set current game of this player
        player.set_game(Some(self.id));
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
        self.set_state(msg.state);
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
        self.push_all(&Packet::notify(
            game_manager::COMPONENT,
            game_manager::GAME_SETTINGS_CHANGE,
            SettingChange {
                id: self.id,
                setting,
            },
        ));
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
            game_manager::COMPONENT,
            game_manager::GAME_ATTRIB_CHANGE,
            AttributesChange {
                id: self.id,
                attributes: &attributes,
            },
        );

        self.attributes.insert_presorted(attributes.into_inner());
        self.push_all(&packet);

        // Don't update matchmaking for full games
        if self.players.len() < Self::MAX_PLAYERS {
            let game_manager = self.game_manager.clone();
            let game_link = ctx.link();
            let game_id = self.id;
            tokio::spawn(async move {
                game_manager.process_queue(game_link, game_id).await;
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
        self.rem_user_sub(&player);
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
#[msg(rtype = "RawBlaze")]
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
        let data: RawBlaze = data.into();
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

    fn set_state(&mut self, state: GameState) {
        self.state = state;
        self.push_all(&Packet::notify(
            game_manager::COMPONENT,
            game_manager::GAME_STATE_CHANGE,
            StateChange {
                id: self.id,
                state: self.state,
            },
        ));
    }

    /// Creates a subscription between all the users and the the target player
    fn add_user_sub(&self, target_id: PlayerID, target_link: SessionLink) {
        debug!("Adding user subscriptions");

        // Subscribe all the clients to eachother
        self.players
            .iter()
            .filter(|other| other.player.id.ne(&target_id))
            .for_each(|other| {
                _ = target_link
                    .do_send(SubscriberMessage::Sub(other.player.id, other.link.clone()));

                _ = other
                    .link
                    .do_send(SubscriberMessage::Sub(target_id, target_link.clone()));
            });
    }

    /// Notifies the provided player and all other players
    /// in the game that they should remove eachother from
    /// their player data list
    fn rem_user_sub(&self, player: &GamePlayer) {
        debug!("Removing user subscriptions");

        // Unsubscribe all the clients from eachother
        self.players
            .iter()
            .filter(|other| other.player.id.ne(&player.player.id))
            .for_each(|other| {
                _ = player
                    .link
                    .do_send(SubscriberMessage::Remove(other.player.id));

                _ = other
                    .link
                    .do_send(SubscriberMessage::Remove(player.player.id));
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
