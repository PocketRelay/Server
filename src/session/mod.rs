//! Sessions are client connections to the main server with associated
//! data such as player data for when they become authenticated and
//! networking data.

use self::{
    models::user_sessions::{
        HardwareFlags, LookupResponse, NotifyUserAdded, NotifyUserRemoved, NotifyUserUpdated,
        UserDataFlags, UserIdentification, UserSessionExtendedData, UserSessionExtendedDataUpdate,
    },
    packet::{Packet, PacketCodec, PacketDebug},
    router::BlazeRouter,
};
use crate::{
    database::entities::Player,
    services::{game::manager::GameManager, sessions::Sessions},
    session::models::{NetworkAddress, QosNetworkData},
    utils::{
        components::{self, user_sessions},
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
use std::{fmt::Debug, net::Ipv4Addr, sync::Arc};
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
    game_manager: Arc<GameManager>,
    sessions: Arc<Sessions>,
}

pub struct SessionExtData {
    player: Arc<Player>,
    net: Arc<NetData>,
    game: Option<GameID>,
    subscribers: Vec<(PlayerID, SessionLink)>,
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
            game: self.game,
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

            self.link.debug_log_packet("Queued Write", &packet).await;
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
                link.debug_log_packet("Read", &packet).await;
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
    pub fn start(
        id: SessionID,
        io: Upgraded,
        addr: Ipv4Addr,
        router: Arc<BlazeRouter>,
        game_manager: Arc<GameManager>,
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
            game_manager,
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

        let session: Self = match Arc::try_unwrap(self) {
            Ok(value) => value,
            Err(arc) => {
                let references = Arc::strong_count(&arc);
                warn!(
                    "Session {} was stopped but {} references to it still exist",
                    arc.id, references
                );
                return;
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

    pub async fn clear_player(&self) {
        // Check that theres authentication
        let data = &mut *self.data.write().await;
        let data = match data {
            Some(value) => value,
            None => return,
        };

        // Existing sessions must be unsubscribed
        data.subscribers.clear();

        // Remove session from games service
        self.game_manager
            .remove_session(data.game.take(), data.player.id)
            .await;

        // Remove the session from the sessions service
        self.sessions.remove_session(data.player.id).await;
    }

    pub async fn get_game(&self) -> Option<GameID> {
        let data = &*self.data.read().await;
        data.as_ref().and_then(|value| value.game)
    }

    pub async fn take_game(&self) -> Option<GameID> {
        let data = &mut *self.data.write().await;
        data.as_mut().and_then(|value| value.game.take())
    }

    pub async fn get_lookup(&self) -> Option<LookupResponse> {
        let data = &*self.data.read().await;
        data.as_ref().map(|data| LookupResponse {
            player: data.player.clone(),
            extended_data: UserSessionExtendedData {
                net: data.net.clone(),
                game: data.game,
            },
        })
    }

    pub async fn set_game(&self, game: Option<GameID>) {
        let data = &mut *self.data.write().await;
        let data = match data {
            Some(value) => value,
            // TODO: Handle this as an error for unauthenticated
            None => return,
        };

        data.game = game;
        data.publish_update();
    }

    pub async fn set_hardware_flags(&self, value: HardwareFlags) {
        let data = &mut *self.data.write().await;
        let data = match data {
            Some(value) => value,
            // TODO: Handle this as an error for unauthenticated
            None => return,
        };

        data.net = Arc::new(data.net.with_hardware_flags(value));
        data.publish_update();
    }

    pub async fn set_network_info(&self, address: NetworkAddress, qos: QosNetworkData) {
        let data = &mut *self.data.write().await;
        let data = match data {
            Some(value) => value,
            // TODO: Handle this as an error for unauthenticated
            None => return,
        };

        data.net = Arc::new(data.net.with_basic(address, qos));
        data.publish_update();
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

        // Ping messages are ignored from debug logging as they are very frequent
        let ignored = packet.header.component == components::util::COMPONENT
            && (packet.header.command == components::util::PING
                || packet.header.command == components::util::SUSPEND_USER_PING);

        if ignored {
            return;
        }
        // TODO: Blocking read is currently the only known solution
        let data = &*self.data.read().await;
        let debug = SessionPacketDebug {
            action,
            packet,
            session_id: self.id,
            session_data: data,
        };

        debug!("\n{:?}", debug);
    }
}

/// Structure for wrapping session details around a debug
/// packet message for logging
struct SessionPacketDebug<'a> {
    action: &'static str,
    packet: &'a Packet,
    session_id: SessionID,
    session_data: &'a Option<SessionExtData>,
}

impl Debug for SessionPacketDebug<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Session {} Packet", self.action)?;

        if let Some(data) = &self.session_data {
            writeln!(
                f,
                "Info: (Name: {}, ID: {}, SID: {})",
                &data.player.display_name, data.player.id, &self.session_id
            )?;
        } else {
            writeln!(f, "Info: (SID: {})", &self.session_id)?;
        }

        let header = &self.packet.header;

        let minified = (header.component == components::authentication::COMPONENT
            && header.command == components::authentication::LIST_USER_ENTITLEMENTS_2)
            || (header.component == components::util::COMPONENT
                && (header.command == components::util::FETCH_CLIENT_CONFIG
                    || header.command == components::util::USER_SETTINGS_LOAD_ALL));

        PacketDebug {
            packet: self.packet,
            minified,
        }
        .fmt(f)
    }
}
