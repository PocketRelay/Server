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
        game::{GameRef, WeakGameRef},
        sessions::Sessions,
    },
    session::models::{NetworkAddress, QosNetworkData},
    utils::{
        components::{component_key, user_sessions, DEBUG_IGNORED_PACKETS},
        lock::{QueueLock, QueueLockGuard, TicketAquireFuture},
        types::{GameID, PlayerID},
    },
};
use futures_util::{future::BoxFuture, Sink, Stream};
use hyper::upgrade::Upgraded;
use log::{debug, log_enabled, warn};
use parking_lot::Mutex;
use serde::Serialize;
use std::{
    fmt::Debug,
    net::Ipv4Addr,
    pin::Pin,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    task::{ready, Context, Poll},
};
use std::{future::Future, sync::Weak};
use tokio::sync::mpsc;
use tokio_util::codec::Framed;

pub mod models;
pub mod packet;
pub mod router;
pub mod routes;

pub type SessionLink = Arc<Session>;
pub type WeakSessionLink = Weak<Session>;

pub struct Session {
    id: u32,
    addr: Ipv4Addr,
    busy_lock: QueueLock,
    tx: mpsc::UnboundedSender<Packet>,
    data: Mutex<Option<SessionExtData>>,
    sessions: Arc<Sessions>,
}

#[derive(Clone)]
pub struct SessionNotifyHandle {
    busy_lock: QueueLock,
    tx: mpsc::UnboundedSender<Packet>,
}

impl SessionNotifyHandle {
    /// Pushes a new notification packet, this will aquire a queue position
    /// waiting until the current response is handled before sending
    pub fn notify(&self, packet: Packet) {
        let tx = self.tx.clone();
        let busy_lock = self.busy_lock.aquire();
        tokio::spawn(async move {
            let _guard = busy_lock.await;
            let _ = tx.send(packet);
        });
    }
}

pub struct SessionExtData {
    player: Arc<Player>,
    net: Arc<NetData>,
    game: Option<SessionGameData>,
    subscribers: Vec<(PlayerID, SessionNotifyHandle)>,
}

struct SessionGameData {
    game_id: GameID,
    game_ref: WeakGameRef,
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

    fn add_subscriber(&mut self, player_id: PlayerID, subscriber: SessionNotifyHandle) {
        // Notify the addition of this user data to the subscriber
        subscriber.notify(Packet::notify(
            user_sessions::COMPONENT,
            user_sessions::USER_ADDED,
            NotifyUserAdded {
                session_data: self.ext(),
                user: UserIdentification::from_player(&self.player),
            },
        ));

        // Notify the user that they are now subscribed to this user
        subscriber.notify(Packet::notify(
            user_sessions::COMPONENT,
            user_sessions::USER_UPDATED,
            NotifyUserUpdated {
                flags: UserDataFlags::SUBSCRIBED | UserDataFlags::ONLINE,
                player_id: self.player.id,
            },
        ));

        // Add the subscriber
        self.subscribers.push((player_id, subscriber));
    }

    fn remove_subscriber(&mut self, player_id: PlayerID) {
        let subscriber = self
            .subscribers
            .iter()
            // Find the subscriber to remove
            .position(|(id, _sub)| player_id.eq(id))
            // Remove the subscriber
            .map(|index| self.subscribers.swap_remove(index));

        if let Some((_, subscriber)) = subscriber {
            // Notify the subscriber they've removed the user subcription
            subscriber.notify(Packet::notify(
                user_sessions::COMPONENT,
                user_sessions::USER_REMOVED,
                NotifyUserRemoved { player_id },
            ))
        }
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

        self.subscribers
            .iter()
            .for_each(|(_, sub)| sub.notify(packet.clone()));
    }
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct NetData {
    pub addr: NetworkAddress,
    pub qos: QosNetworkData,
    pub hardware_flags: HardwareFlags,
    pub ping_site_latency: Vec<u32>,
}

impl NetData {
    // Re-creates the current net data using the provided address and QOS data
    pub fn with_basic(
        &self,
        addr: NetworkAddress,
        qos: QosNetworkData,
        ping_site_latency: Vec<u32>,
    ) -> Self {
        Self {
            addr,
            qos,
            hardware_flags: self.hardware_flags,
            ping_site_latency,
        }
    }

    /// Re-creates the current net data using the provided hardware flags
    pub fn with_hardware_flags(&self, flags: HardwareFlags) -> Self {
        Self {
            addr: self.addr.clone(),
            qos: self.qos,
            hardware_flags: flags,
            ping_site_latency: self.ping_site_latency.clone(),
        }
    }
}

static SESSION_IDS: AtomicU32 = AtomicU32::new(1);

impl Session {
    pub async fn start(
        io: Upgraded,
        addr: Ipv4Addr,
        router: Arc<BlazeRouter>,
        sessions: Arc<Sessions>,
    ) {
        // Obtain a session ID
        let id = SESSION_IDS.fetch_add(1, Ordering::AcqRel);

        let (tx, rx) = mpsc::unbounded_channel();

        let session = Arc::new(Self {
            id,
            busy_lock: QueueLock::new(),
            tx,
            data: Default::default(),
            addr,
            sessions,
        });

        SessionFuture {
            io: Framed::new(io, PacketCodec::default()),
            router: &router,
            rx,
            session: session.clone(),
            read_state: ReadState::Recv,
            write_state: WriteState::Recv,
            stop: false,
        }
        .await;

        session.stop();
    }

    pub fn notify_handle(&self) -> SessionNotifyHandle {
        SessionNotifyHandle {
            busy_lock: self.busy_lock.clone(),
            tx: self.tx.clone(),
        }
    }

    /// Called when the session is considered stopped (Reader/Writer future has completed)
    /// in order to clean up any remaining references to the session before dropping
    fn stop(self: Arc<Self>) {
        // Clear authentication
        self.clear_player();

        // Session should now be the sole owner
        let session = match Arc::try_unwrap(self) {
            Ok(value) => value,
            Err(arc) => {
                let references = Arc::strong_count(&arc);
                warn!(
                    "Failed to stop session {} there are still {} references to it",
                    arc.id, references
                );
                return;
            }
        };

        debug!("Session stopped (SID: {})", session.id);
    }

    pub fn add_subscriber(&self, player_id: PlayerID, subscriber: SessionNotifyHandle) {
        let data = &mut *self.data.lock();
        if let Some(data) = data {
            data.add_subscriber(player_id, subscriber);
        }
    }
    pub fn remove_subscriber(&self, player_id: PlayerID) {
        let data = &mut *self.data.lock();
        if let Some(data) = data {
            data.remove_subscriber(player_id)
        }
    }

    pub fn set_player(&self, player: Player) -> Arc<Player> {
        // Clear the current authentication
        self.clear_player();

        let data = &mut *self.data.lock();
        let data = data.insert(SessionExtData::new(player));

        data.player.clone()
    }

    /// Clears the current game returning the game data if
    /// the player was in a game
    ///
    /// Called by the game itself when the player has been removed
    pub fn clear_game(&self) -> Option<(PlayerID, WeakGameRef)> {
        let mut game: Option<SessionGameData> = None;
        let mut player_id: Option<PlayerID> = None;

        self.update_data(|data| {
            game = data.game.take();
            player_id = Some(data.player.id);
        });

        let game = game?;
        let player_id = player_id?;

        Some((player_id, game.game_ref))
    }

    /// Called to remove the player from its current game
    pub fn remove_from_game(&self) {
        let (player_id, game_ref) = match self.clear_game() {
            Some(value) => value,
            // Player isn't in a game
            None => return,
        };

        let game_ref = match game_ref.upgrade() {
            Some(value) => value,
            // Game doesn't exist anymore
            None => return,
        };

        // Spawn an async task to handle removing the player
        tokio::spawn(async move {
            let game = &mut *game_ref.write().await;
            game.remove_player(player_id, RemoveReason::PlayerLeft);
        });
    }

    pub fn clear_player(&self) {
        self.remove_from_game();

        // Check that theres authentication
        let data = &mut *self.data.lock();
        let data = match data {
            Some(value) => value,
            None => return,
        };

        // Existing sessions must be unsubscribed
        data.subscribers.clear();

        // Remove the session from the sessions service
        self.sessions.remove_session(data.player.id);
    }

    pub fn get_game(&self) -> Option<(GameID, GameRef)> {
        let data = &*self.data.lock();
        data.as_ref()
            .and_then(|value| value.game.as_ref())
            // Try upgrading the reference to get an actual game
            .and_then(|value| {
                value
                    .game_ref
                    .upgrade()
                    .map(|game_ref| (value.game_id, game_ref))
            })
    }

    pub fn get_lookup(&self) -> Option<LookupResponse> {
        let data = &*self.data.lock();
        data.as_ref().map(|data| LookupResponse {
            player: data.player.clone(),
            extended_data: data.ext(),
        })
    }

    #[inline]
    fn update_data<F>(&self, update: F)
    where
        F: FnOnce(&mut SessionExtData),
    {
        let data = &mut *self.data.lock();
        if let Some(data) = data {
            update(data);
            data.publish_update();
        }
    }

    pub fn set_game(&self, game_id: GameID, game_ref: WeakGameRef) {
        // Remove the player from the game if they are already present in one
        self.remove_from_game();

        // Set the current game
        self.update_data(|data| {
            data.game = Some(SessionGameData { game_id, game_ref });
        });
    }

    #[inline]
    pub fn set_hardware_flags(&self, value: HardwareFlags) {
        self.update_data(|data| {
            data.net = Arc::new(data.net.with_hardware_flags(value));
        });
    }

    pub fn network_info(&self) -> Option<Arc<NetData>> {
        let data = &mut *self.data.lock();
        data.as_ref().map(|value| value.net.clone())
    }

    #[inline]
    pub fn set_network_info(
        &self,
        address: NetworkAddress,
        qos: QosNetworkData,
        ping_site_latency: Vec<u32>,
    ) {
        self.update_data(|data| {
            data.net = Arc::new(data.net.with_basic(address, qos, ping_site_latency));
        });
    }

    /// Logs the contents of the provided packet to the debug output along with
    /// the header information and basic session information.
    ///
    /// `action` The name of the action this packet is undergoing.
    ///          (e.g. Writing or Reading)
    /// `packet` The packet that is being logged
    fn debug_log_packet(&self, action: &'static str, packet: &Packet) {
        // Skip if debug logging is disabled
        if !log_enabled!(log::Level::Debug) {
            return;
        }

        let key = component_key(packet.frame.component, packet.frame.command);

        // Don't log the packet if its debug ignored
        if DEBUG_IGNORED_PACKETS.contains(&key) {
            return;
        }

        // Get the authenticated player to include in the debug message
        let auth = self.data.lock().as_ref().map(|data| data.player.clone());

        let debug_data = DebugSessionData {
            action,
            id: self.id,
            auth,
        };
        let debug_packet = PacketDebug { packet };

        debug!("\n{:?}{:?}", debug_data, debug_packet);
    }
}

struct DebugSessionData {
    id: u32,
    auth: Option<Arc<Player>>,
    action: &'static str,
}

impl Debug for DebugSessionData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Session ({}): {}", self.id, self.action)?;

        if let Some(auth) = &self.auth {
            writeln!(f, "Auth ({}): (Name: {})", auth.id, &auth.display_name,)?;
        }

        Ok(())
    }
}

/// Future for processing a session
struct SessionFuture<'a> {
    /// The IO for reading and writing
    io: Framed<Upgraded, PacketCodec>,
    /// Receiver for packets to write
    rx: mpsc::UnboundedReceiver<Packet>,
    /// The session this link is for
    session: SessionLink,
    /// The router to use
    router: &'a BlazeRouter,
    /// The reading state
    read_state: ReadState<'a>,
    /// The writing state
    write_state: WriteState,
    /// Whether the future has been stopped
    stop: bool,
}

/// Session future writing state
enum WriteState {
    /// Waiting for a packet to write
    Recv,
    /// Waiting for the framed to become read
    Write { packet: Option<Packet> },
    /// Flushing the framed
    Flush,
}

/// Session future reading state
enum ReadState<'a> {
    /// Waiting for a packet
    Recv,
    /// Aquiring a lock guard
    Aquire {
        /// Future for the locking guard
        ticket: TicketAquireFuture,
        /// The packet that was read
        packet: Option<Packet>,
    },
    /// Future for a handler is being polled
    Handle {
        /// Locking guard
        guard: QueueLockGuard,
        /// Handle future
        future: BoxFuture<'a, Packet>,
    },
}

impl SessionFuture<'_> {
    /// Polls the write state, the poll ready state returns whether
    /// the future should continue
    fn poll_write_state(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        match &mut self.write_state {
            WriteState::Recv => {
                // Try receive a packet from the write channel
                let result = ready!(Pin::new(&mut self.rx).poll_recv(cx));

                if let Some(packet) = result {
                    self.write_state = WriteState::Write {
                        packet: Some(packet),
                    };
                } else {
                    // All writers have closed, session must be closed (Future end)
                    self.stop = true;
                }
            }
            WriteState::Write { packet } => {
                // Wait until the inner is ready
                if ready!(Pin::new(&mut self.io).poll_ready(cx)).is_ok() {
                    let packet = packet
                        .take()
                        .expect("Unexpected write state without packet");

                    self.session.debug_log_packet("Send", &packet);

                    // Write the packet to the buffer
                    Pin::new(&mut self.io)
                        .start_send(packet)
                        // Packet encoder impl shouldn't produce errors
                        .expect("Packet encoder errored");

                    self.write_state = WriteState::Flush;
                } else {
                    // Failed to ready, session must be closed
                    self.stop = true;
                }
            }
            WriteState::Flush => {
                // Wait until the flush is complete
                if ready!(Pin::new(&mut self.io).poll_flush(cx)).is_ok() {
                    self.write_state = WriteState::Recv;
                } else {
                    // Failed to flush, session must be closed
                    self.stop = true
                }
            }
        }

        Poll::Ready(())
    }

    /// Polls the read state, the poll ready state returns whether
    /// the future should continue
    fn poll_read_state(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        match &mut self.read_state {
            ReadState::Recv => {
                // Try receive a packet from the write channel
                let result = ready!(Pin::new(&mut self.io).poll_next(cx));

                if let Some(Ok(packet)) = result {
                    let ticket = self.session.busy_lock.aquire();
                    self.read_state = ReadState::Aquire {
                        ticket,
                        packet: Some(packet),
                    }
                } else {
                    // Reader has closed or reading encountered an error (Either way stop reading)
                    self.stop = true;
                }
            }
            ReadState::Aquire { ticket, packet } => {
                let guard = ready!(Pin::new(ticket).poll(cx));
                let packet = packet
                    .take()
                    .expect("Unexpected aquire state without packet");

                self.session.debug_log_packet("Receive", &packet);

                let future = self.router.handle(self.session.clone(), packet);

                // Move onto a handling state
                self.read_state = ReadState::Handle { guard, future };
            }
            ReadState::Handle {
                guard: _gaurd,
                future,
            } => {
                // Poll the handler until completion
                let response = ready!(Pin::new(future).poll(cx));

                // Send the response to the writer
                _ = self.session.tx.send(response);

                // Reset back to the reading state
                self.read_state = ReadState::Recv;
            }
        }
        Poll::Ready(())
    }
}

impl Future for SessionFuture<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        while this.poll_write_state(cx).is_ready() {}
        while this.poll_read_state(cx).is_ready() {}

        if this.stop {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
