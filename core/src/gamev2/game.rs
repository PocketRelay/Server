use blaze_pk::{codec::Codec, packet::Packet, types::TdfMap};
use futures::{future::join, Future};
use log::debug;
use tokio::{join, sync::RwLock};

use crate::blaze::{
    components::{Components, GameManager},
    session::SessionArc,
};

use super::codec::{AttributesChange, PlayerJoining, SettingChange, StateChange};

pub struct Game {
    /// Unique ID for this game
    pub id: u32,
    /// Mutable data for this game
    pub data: RwLock<GameData>,
    /// The list of players in this game
    pub players: RwLock<Vec<SessionArc>>,
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

    /// Writes all the provided packets to all connected sessions.
    /// Does not wait for the write to complete just waits for
    /// it to be placed into each sessions write buffers.
    ///
    /// `packets` The packets to write
    async fn write_all_list(&self, packets: &Vec<Packet>) {
        let players = &*self.players.read().await;
        let futures = players
            .iter()
            .map(|value| value.write_all(packets))
            .collect::<Vec<_>>();

        let _ = futures::future::join_all(futures).await;
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
    async fn set_state(&self, state: u16) {
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
    async fn set_setting(&self, setting: u16) {
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
    async fn set_attributes(&self, attributes: AttrMap) {
        debug!("Updating game attributes");
        let data = &mut *self.data.write().await;
        data.attributes = attributes;
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

    // Attempts to add a player to the game. Will return false if
    // the player could not be added because the game is full
    pub async fn try_add_player(&self, session: &SessionArc) -> bool {
        let slot = self.player_count().await;

        self.notify_player_joining(session, slot).await;
        self.update_clients(session).await;

        {
            let players = &mut *self.players.write().await;
            players.push(session.clone());
        }

        session.set_game(self.id).await;

        true
    }

    /// Notifies all the players in the game that a new player has
    /// joined the game.
    pub async fn notify_player_joining(&self, session: &SessionArc, slot: usize) {
        let session_data = &*session.data.read().await;
        let packet = Packet::notify(
            Components::GameManager(GameManager::PlayerJoining),
            &PlayerJoining {
                id: self.id,
                slot,
                session: &session_data,
            },
        );
        self.write_all(&packet).await;
    }
}
