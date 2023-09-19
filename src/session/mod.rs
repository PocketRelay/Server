//! Sessions are client connections to the main server with associated
//! data such as player data for when they become authenticated and
//! networking data.

use self::{
    models::{
        game_manager::RemoveReason,
        user_sessions::{
            HardwareFlags, LookupResponse, NotifyUserAdded, NotifyUserRemoved, NotifyUserUpdated,
            UserDataFlags, UserIdentification, UserSessionExtendedData,
            UserSessionExtendedDataUpdate,
        },
    },
    packet::{Packet, PacketCodec, PacketDebug},
    router::BlazeRouter,
};
use crate::{
    database::entities::Player,
    services::{
        game::{Game, GameRef},
        sessions::Sessions,
    },
    session::models::{NetworkAddress, QosNetworkData},
    utils::{
        components::{component_key, user_sessions, DEBUG_IGNORED_PACKETS},
        types::{GameID, PlayerID, SessionID},
    },
};
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use hyper::upgrade::Upgraded;
use log::{debug, log_enabled, warn};
use serde::Serialize;
use std::{fmt::Debug, net::Ipv4Addr, sync::Arc, time::Duration};
use tokio::{
    sync::{mpsc, RwLock},
    task::JoinSet,
};
use tokio_util::codec::Framed;

pub mod models;
pub mod packet;
pub mod router;
pub mod routes;

pub type SessionLink = Arc<Session>;

pub struct Session {
    id: SessionID,
    addr: Ipv4Addr,
    writer: mpsc::UnboundedSender<WriteMessage>,
    data: RwLock<Option<SessionExtData>>,
    router: Arc<BlazeRouter>,
    sessions: Arc<Sessions>,
}

pub struct SessionExtData {
    player: Arc<Player>,
    net: Arc<NetData>,
    game: Option<SessionGameData>,
    subscribers: Vec<(PlayerID, SessionLink)>,
}

struct SessionGameData {
    game_id: GameID,
    game_ref: Arc<RwLock<Game>>,
}

impl SessionExtData {
    pub fn new(player: Player) -> Self {
        Self {
            player: Arc::new(player),
            net: Default::default(),
            game: Default::default(),
            subscribers: Default::default(),
        }
    }

    fn ext(&self) -> UserSessionExtendedData {
        UserSessionExtendedData {
            net: self.net.clone(),
            game: self.game.as_ref().map(|game| game.game_id),
        }
    }

    fn add_subscriber(&mut self, player_id: PlayerID, subscriber: SessionLink) {
        // Create the details packets
        let added_notify = Packet::notify(
            user_sessions::COMPONENT,
            user_sessions::USER_ADDED,
            NotifyUserAdded {
                session_data: self.ext(),
                user: UserIdentification::from_player(&self.player),
            },
        );

        // Create update notifying the user of the subscription
        let update_notify = Packet::notify(
            user_sessions::COMPONENT,
            user_sessions::USER_UPDATED,
            NotifyUserUpdated {
                flags: UserDataFlags::SUBSCRIBED | UserDataFlags::ONLINE,
                player_id: self.player.id,
            },
        );

        self.subscribers.push((player_id, subscriber.clone()));
        subscriber.push(added_notify);
        subscriber.push(update_notify);
    }

    fn remove_subscriber(&mut self, player_id: PlayerID) {
        let index = match self.subscribers.iter().position(|(id, _)| player_id.eq(id)) {
            Some(value) => value,
            None => return,
        };

        let (_, subscriber) = self.subscribers.swap_remove(index);

        // Create the details packets
        let removed_notify = Packet::notify(
            user_sessions::COMPONENT,
            user_sessions::USER_REMOVED,
            NotifyUserRemoved { player_id },
        );

        subscriber.push(removed_notify)
    }

    /// Publishes changes of the session data to all the
    /// subscribed session links
    fn publish_update(&self) {
        let packet = Packet::notify(
            user_sessions::COMPONENT,
            user_sessions::USER_SESSION_EXTENDED_DATA_UPDATE,
            UserSessionExtendedDataUpdate {
                user_id: self.player.id,
                data: self.ext(),
            },
        );

        for (_, subscriber) in &self.subscribers {
            subscriber.push(packet.clone());
        }
    }
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct NetData {
    pub addr: NetworkAddress,
    pub qos: QosNetworkData,
    pub hardware_flags: HardwareFlags,
}

impl NetData {
    // Re-creates the current net data using the provided address and QOS data
    pub fn with_basic(&self, addr: NetworkAddress, qos: QosNetworkData) -> Self {
        Self {
            addr,
            qos,
            hardware_flags: self.hardware_flags,
        }
    }

    /// Re-creates the current net data using the provided hardware flags
    pub fn with_hardware_flags(&self, flags: HardwareFlags) -> Self {
        Self {
            addr: self.addr.clone(),
            qos: self.qos,
            hardware_flags: flags,
        }
    }
}

// Writer for writing packets
struct SessionWriter {
    inner: SplitSink<Framed<Upgraded, PacketCodec>, Packet>,
    rx: mpsc::UnboundedReceiver<WriteMessage>,
    link: SessionLink,
}

pub enum WriteMessage {
    Write(Packet),
    Close,
}

impl SessionWriter {
    pub async fn process(mut self) {
        while let Some(msg) = self.rx.recv().await {
            let packet = match msg {
                WriteMessage::Write(packet) => packet,
                WriteMessage::Close => break,
            };

            self.link.debug_log_packet("Send", &packet).await;
            if self.inner.send(packet).await.is_err() {
                break;
            }
        }
    }
}

struct SessionReader {
    inner: SplitStream<Framed<Upgraded, PacketCodec>>,
    link: SessionLink,
}

impl SessionReader {
    pub async fn process(mut self) {
        let mut tasks = JoinSet::new();

        while let Some(Ok(packet)) = self.inner.next().await {
            let link = self.link.clone();
            tasks.spawn(async move {
                link.debug_log_packet("Receive", &packet).await;
                let response = match link.router.handle(link.clone(), packet) {
                    // Await the handler response future
                    Ok(fut) => fut.await,

                    // Handle no handler for packet
                    Err(packet) => {
                        debug!("Missing packet handler");
                        Packet::response_empty(&packet)
                    }
                };
                // Push the response to the client
                link.push(response);
            });
        }

        tasks.shutdown().await;

        self.link.stop().await;
    }
}

impl Session {
    /// Max number of times to poll a session for shutdown before erroring
    const MAX_RELEASE_ATTEMPTS: u8 = 5;

    pub fn start(
        id: SessionID,
        io: Upgraded,
        addr: Ipv4Addr,
        router: Arc<BlazeRouter>,
        sessions: Arc<Sessions>,
    ) {
        let framed = Framed::new(io, PacketCodec);
        let (write, read) = framed.split();
        let (tx, rx) = mpsc::unbounded_channel();

        let session = Arc::new(Self {
            id,
            writer: tx,
            data: Default::default(),
            addr,
            router,
            sessions,
        });

        let reader = SessionReader {
            link: session.clone(),
            inner: read,
        };

        let writer = SessionWriter {
            link: session.clone(),
            rx,
            inner: write,
        };

        tokio::spawn(reader.process());
        tokio::spawn(writer.process());
    }

    /// Internal session stopped function called by the reader when
    /// the connection is terminated, cleans up any references and
    /// asserts only 1 strong reference exists
    async fn stop(self: Arc<Self>) {
        // Tell the write half to close and wait until its closed
        _ = self.writer.send(WriteMessage::Close);
        self.writer.closed().await;

        // Clear authentication
        self.clear_player().await;

        let mut attempt: u8 = 1;

        let mut arc = self;
        let session = loop {
            if attempt > Self::MAX_RELEASE_ATTEMPTS {
                let references = Arc::strong_count(&arc);
                warn!(
                    "Failed to stop session {} there are still {} references to it",
                    arc.id, references
                );
                return;
            }
            match Arc::try_unwrap(arc) {
                Ok(value) => break value,
                Err(value) => {
                    let wait = 5 * attempt as u64;
                    let references = Arc::strong_count(&value);
                    debug!(
                        "Session {} still has {} references to it, waiting {}s",
                        value.id, references, wait
                    );
                    tokio::time::sleep(Duration::from_secs(wait)).await;
                    arc = value;
                    attempt += 1;
                    continue;
                }
            }
        };

        debug!("Session stopped (SID: {})", session.id);
    }

    pub async fn add_subscriber(&self, player_id: PlayerID, subscriber: SessionLink) {
        let data = &mut *self.data.write().await;
        let data = match data {
            Some(value) => value,
            // TODO: Handle this as an error for unauthenticated
            None => return,
        };
        data.add_subscriber(player_id, subscriber);
    }

    pub async fn remove_subscriber(&self, player_id: PlayerID) {
        let data = &mut *self.data.write().await;
        let data = match data {
            Some(value) => value,
            // TODO: Handle this as an error for unauthenticated
            None => return,
        };
        data.remove_subscriber(player_id);
    }

    pub async fn set_player(&self, player: Player) -> Arc<Player> {
        // Clear the current authentication
        self.clear_player().await;

        let data = &mut *self.data.write().await;
        let data = data.insert(SessionExtData::new(player));

        data.player.clone()
    }

    /// Clears the current game returning the game data if
    /// the player was in a game
    ///
    /// Called by the game itself when the player has been removed
    pub async fn clear_game(&self) -> Option<(PlayerID, GameRef)> {
        // Check that theres authentication
        let data = &mut *self.data.write().await;
        let data = data.as_mut()?;
        let game = data.game.take();
        data.publish_update();
        let game = game?;

        Some((data.player.id, game.game_ref))
    }

    /// Called to remove the player from its current game
    pub async fn remove_from_game(&self) {
        if let Some((player_id, game_ref)) = self.clear_game().await {
            let game = &mut *game_ref.write().await;
            game.remove_player(player_id, RemoveReason::PlayerLeft);
        }
    }

    pub async fn clear_player(&self) {
        self.remove_from_game().await;

        // Check that theres authentication
        let data = &mut *self.data.write().await;
        let data = match data {
            Some(value) => value,
            None => return,
        };

        // Existing sessions must be unsubscribed
        data.subscribers.clear();

        // Remove the session from the sessions service
        self.sessions.remove_session(data.player.id).await;
    }

    pub async fn get_game(&self) -> Option<(GameID, GameRef)> {
        let data = &*self.data.read().await;
        data.as_ref()
            .and_then(|value| value.game.as_ref())
            .map(|value| (value.game_id, value.game_ref.clone()))
    }

    pub async fn get_lookup(&self) -> Option<LookupResponse> {
        let data = &*self.data.read().await;
        data.as_ref().map(|data| LookupResponse {
            player: data.player.clone(),
            extended_data: data.ext(),
        })
    }

    #[inline]
    async fn update_data<F>(&self, update: F)
    where
        F: FnOnce(&mut SessionExtData),
    {
        let data = &mut *self.data.write().await;
        if let Some(data) = data {
            update(data);
            data.publish_update();
        }
    }

    pub async fn set_game(&self, game_id: GameID, game_ref: GameRef) {
        // Set the current game
        self.update_data(|data| {
            // Remove the player from the game if they are already present in one
            if let Some(game) = data.game.take() {
                let player_id = data.player.id;
                tokio::spawn(async move {
                    let game = &mut *game.game_ref.write().await;
                    game.remove_player(player_id, RemoveReason::PlayerLeft);
                });
            }

            data.game = Some(SessionGameData { game_id, game_ref });
        })
        .await;
    }

    #[inline]
    pub async fn set_hardware_flags(&self, value: HardwareFlags) {
        self.update_data(|data| {
            data.net = Arc::new(data.net.with_hardware_flags(value));
        })
        .await;
    }

    #[inline]
    pub async fn set_network_info(&self, address: NetworkAddress, qos: QosNetworkData) {
        self.update_data(|data| {
            data.net = Arc::new(data.net.with_basic(address, qos));
        })
        .await;
    }

    /// Pushes a new packet to the back of the packet buffer
    /// and sends a flush notification
    ///
    /// `packet` The packet to push to the buffer
    pub fn push(&self, packet: Packet) {
        _ = self.writer.send(WriteMessage::Write(packet))
        // TODO: Handle failing to send contents to writer
    }

    /// Logs the contents of the provided packet to the debug output along with
    /// the header information and basic session information.
    ///
    /// `action` The name of the action this packet is undergoing.
    ///          (e.g. Writing or Reading)
    /// `packet` The packet that is being logged
    async fn debug_log_packet(&self, action: &'static str, packet: &Packet) {
        // Skip if debug logging is disabled
        if !log_enabled!(log::Level::Debug) {
            return;
        }

        let key = component_key(packet.frame.component, packet.frame.command);
        let ignored = DEBUG_IGNORED_PACKETS.contains(&key);
        if ignored {
            return;
        }

        let data = &*self.data.read().await;
        let debug_data = DebugSessionData {
            action,
            id: self.id,
            data,
        };
        let debug_packet = PacketDebug { packet };

        debug!("\n{:?}{:?}", debug_data, debug_packet);
    }
}

struct DebugSessionData<'a> {
    id: SessionID,
    data: &'a Option<SessionExtData>,
    action: &'static str,
}

impl Debug for DebugSessionData<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Session ({}): {}", self.id, self.action)?;

        if let Some(data) = self.data.as_ref() {
            writeln!(
                f,
                "Auth ({}): (Name: {})",
                data.player.id, &data.player.display_name,
            )?;
        }

        Ok(())
    }
}
