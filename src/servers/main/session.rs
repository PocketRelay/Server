//! Sessions are client connections to the main server with associated
//! data such as player data for when they become authenticated and
//! networking data.
use super::models::errors::{ServerError, ServerResult};
use super::router;
use crate::utils::types::{BoxFuture, PlayerID};
use crate::{
    services::game::{player::GamePlayer, RemovePlayerType},
    state::GlobalState,
    utils::{
        components::{self, Components, UserSessions},
        models::{NetData, NetGroups, QosNetworkData, UpdateExtDataAttr},
        types::{GameID, SessionID},
    },
};
use blaze_pk::packet::PacketDebug;
use blaze_pk::{codec::Encodable, tag::TdfType, writer::TdfWriter};
use blaze_pk::{
    packet::{Packet, PacketComponents},
    router::State,
};
use database::Player;
use log::{debug, error, log_enabled};
use std::fmt::Debug;
use std::{collections::VecDeque, net::SocketAddr};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot},
};

/// Structure for storing a client session. This includes the
/// network stream for the client along with global state and
/// other session state.
pub struct Session {
    /// Unique identifier for this session.
    id: SessionID,

    /// Underlying connection stream to client
    write: OwnedWriteHalf,

    /// The socket connection address of the client
    socket_addr: SocketAddr,

    /// If the session is authenticated it will have a linked
    /// player model from the database
    pub player: Option<Player>,

    /// Networking information
    net: NetData,

    /// The id of the game if connected to one
    game: Option<GameID>,

    /// The queue of packets that need to be written
    queue: VecDeque<Packet>,

    /// State determining whether the session has a flush message
    /// already queued in the reciever
    flush_queued: bool,

    /// Internal address used for routing can be cloned and used elsewhere
    addr: SessionAddr,
}

impl State for SessionAddr {}

/// Address to a session which allows manipulating sessions asyncronously
/// using mpsc channels without actually having access to the session itself
#[derive(Clone)]
pub struct SessionAddr {
    /// The ID this session is linked to
    pub id: SessionID,
    /// The sender for sending message to this session
    tx: mpsc::UnboundedSender<Message>,
}

/// Trait representing an action that can be executed
/// using the session
trait SessionAction: Sized + Send + 'static {
    /// Type for the resulting value created from this action
    type Result;

    fn handle(self, session: &mut Session) -> Self::Result;
}

impl<F, R> SessionAction for F
where
    F: FnOnce(&mut Session) -> R + Send + 'static,
    R: 'static,
{
    type Result = R;

    fn handle(self, session: &mut Session) -> Self::Result {
        self(session)
    }
}

struct ActionMessage<A, R> {
    action: A,
    tx: oneshot::Sender<R>,
}

struct LazyActionMessage<A> {
    action: A,
}

trait ActionMessageProxy: Send {
    fn handle(self: Box<Self>, session: &mut Session);
}

impl<A, R> ActionMessageProxy for ActionMessage<A, R>
where
    A: SessionAction<Result = R>,
    R: Send + Sized + 'static,
{
    fn handle(self: Box<Self>, session: &mut Session) {
        let result: R = self.action.handle(session);
        self.tx.send(result).ok();
    }
}
impl<A> ActionMessageProxy for LazyActionMessage<A>
where
    A: SessionAction<Result = ()>,
{
    fn handle(self: Box<Self>, session: &mut Session) {
        self.action.handle(session);
    }
}

impl SessionAddr {
    /// Writes a new packet ot the session
    ///
    /// `packet` The packet to write
    pub fn push(&self, packet: Packet) {
        self.tx
            .send(Message::Packet(PacketMessage::Write(packet)))
            .ok();
    }

    fn read(&self, packet: Packet) -> bool {
        self.tx
            .send(Message::Packet(PacketMessage::Read(packet)))
            .is_ok()
    }

    /// Executes the provided action on the actual session itself will
    /// return an option if the session wasn't able to execute the action
    /// likely because the session has ended.
    ///
    /// `action` The action to execute
    pub async fn exec<A, R>(&self, action: A) -> Option<R>
    where
        A: FnOnce(&mut Session) -> R + Send + 'static,
        R: Send + Sized + 'static,
    {
        let (tx, rx) = oneshot::channel();

        if self
            .tx
            .send(Message::Action(Box::new(ActionMessage { action, tx })))
            .is_err()
        {
            return None;
        }

        rx.await.ok()
    }

    /// Executes the provided action lazily this doesn't require awaiting for
    /// a result type
    ///
    /// `action` The action to execute
    pub fn exec_lazy<A>(&self, action: A)
    where
        A: FnOnce(&mut Session) + Send + 'static,
    {
        self.tx
            .send(Message::Action(Box::new(LazyActionMessage { action })))
            .ok();
    }

    pub fn stop(&self) {
        self.tx.send(Message::Stop).ok();
    }

    pub async fn try_into_player(&self) -> Option<GamePlayer> {
        self.exec(|session| {
            let player = session.player.clone()?;
            Some(GamePlayer::new(
                player,
                session.net.clone(),
                session.addr.clone(),
            ))
        })
        .await
        .flatten()
    }

    /// Attempts to set the current player will return true if successful
    /// or false if the sesson is terminated or another error occurs
    ///
    /// `player` The player to set for this session
    pub async fn set_player(&self, player: Player) -> bool {
        self.exec(|session| {
            session.player = Some(player);
        })
        .await
        .is_some()
    }

    pub async fn get_player(&self) -> ServerResult<Option<Player>> {
        self.exec(|session| session.player.clone())
            .await
            .ok_or(ServerError::ServerUnavailable)
    }

    pub async fn get_player_id(&self) -> Option<u32> {
        self.exec(|session| session.player.as_ref().map(|value| value.id))
            .await
            .flatten()
    }

    pub async fn socket_addr(&self) -> Option<SocketAddr> {
        self.exec(|session| session.socket_addr).await
    }

    pub fn spawn(id: SessionID, values: (TcpStream, SocketAddr)) {
        let (sender, receiver) = mpsc::unbounded_channel();
        let addr = SessionAddr { id, tx: sender };

        let (read, write) = values.0.into_split();
        tokio::spawn(Self::spawn_reader(addr.clone(), read));

        let session = Session::new(id, write, values.1, addr);
        tokio::spawn(session.process(receiver));
    }

    pub async fn spawn_reader(addr: Self, mut read: OwnedReadHalf) {
        loop {
            let packet: Packet = match Packet::read_async(&mut read).await {
                Ok(value) => value,
                Err(_) => {
                    addr.stop();
                    break;
                }
            };

            if !addr.read(packet) {
                break;
            }
        }
    }
}

/// Message for communicating with the spawned session
/// using cloned the sender present on cloned addresses
enum Message {
    /// Request to stop the session
    Stop,

    /// Action execution
    Action(Box<dyn ActionMessageProxy>),

    /// Group of messages relating to packets
    Packet(PacketMessage),
}

/// Group of messages realting to packets
enum PacketMessage {
    /// Request a read packet to be processed
    Read(Packet),

    /// Queues a packet to be written to the outbound queue
    Write(Packet),

    /// Request to tell the session to flush any outbound
    /// packets actually writing them to the socket
    Flush,
}

impl Session {
    /// Creates a new session with the provided values.
    ///
    /// `id`             The unique session ID
    /// `values`         The networking TcpStream and address
    /// `message_sender` The message sender for session messages
    fn new(
        id: SessionID,
        write: OwnedWriteHalf,
        socket_addr: SocketAddr,
        addr: SessionAddr,
    ) -> Self {
        Self {
            id,
            write,
            socket_addr,
            queue: VecDeque::new(),
            player: None,
            net: NetData::default(),
            game: None,
            flush_queued: false,
            addr,
        }
    }

    /// Processing function which handles recieving messages, flush notifications,
    /// reading packets, and handling safe shutdowns for this session. This function
    /// owns the session.
    ///
    /// `message` The receiver for receiving session messages
    async fn process(mut self, mut receiver: mpsc::UnboundedReceiver<Message>) {
        while let Some(msg) = receiver.recv().await {
            match msg {
                Message::Packet(msg) => match msg {
                    PacketMessage::Read(packet) => self.handle_packet(packet),
                    PacketMessage::Write(packet) => self.push(packet),
                    PacketMessage::Flush => self.flush().await,
                },
                Message::Stop => break,
                Message::Action(action) => {
                    action.handle(&mut self);
                }
            }
        }
    }

    /// Attempts to obtain a game player from this session will return None
    /// if this session is not authenticated
    fn try_into_player(&self) -> Option<GamePlayer> {
        let player = self.player.clone()?;
        Some(GamePlayer::new(player, self.net.clone(), self.addr.clone()))
    }

    /// Handles processing a recieved packet from the `process` function.
    /// The buffer is flushed after routing is complete.
    ///
    /// `session`   The session to process the packet for
    /// `component` The component of the packet for routing
    /// `packet`    The packet itself
    fn handle_packet(&mut self, packet: Packet) {
        self.debug_log_packet("Read", &packet);
        let mut addr = self.addr.clone();
        tokio::spawn(async move {
            let router = router();
            match router.handle(&mut addr, packet).await {
                Ok(packet) => {
                    addr.push(packet);
                }
                Err(err) => {
                    error!("Error occurred while decoding packet: {:?}", err);
                }
            }
        });
    }

    /// Pushes a new packet to the back of the packet buffer
    /// and sends a flush notification
    ///
    /// `packet` The packet to push to the buffer
    pub fn push(&mut self, packet: Packet) {
        self.queue.push_back(packet);
        self.queue_flush();
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

        let component = Components::from_header(&packet.header);

        // Ping messages are ignored from debug logging as they are very frequent
        let ignored = matches!(
            component,
            Components::Util(components::Util::Ping)
                | Components::Util(components::Util::SuspendUserPing)
        );

        if ignored {
            return;
        }

        let debug = SessionPacketDebug {
            action,
            packet,
            component,
            session: self,
        };

        debug!("\n{:?}", debug);
    }

    /// Queues a new flush if there is not already one queued
    fn queue_flush(&mut self) {
        if !self.flush_queued {
            self.flush_queued = true;
            self.addr
                .tx
                .send(Message::Packet(PacketMessage::Flush))
                .ok();
        }
    }

    /// Flushes the output buffer
    async fn flush(&mut self) {
        self.flush_queued = false;

        // Counter for the number of items written
        let mut write_count = 0usize;
        while let Some(item) = self.queue.pop_front() {
            self.debug_log_packet("Wrote", &item);
            match item.write_async(&mut self.write).await {
                Ok(_) => {
                    write_count += 1;
                }
                Err(err) => {
                    error!(
                        "Error occurred while flushing session (SID: {}): {:?}",
                        self.id, err
                    );
                    return;
                }
            }
        }

        if write_count > 0 {
            debug!("Flushed session (SID: {}, Count: {})", self.id, write_count);
        }
    }

    /// Sets the game details for the current session and updates
    /// the client with the new sesion details
    ///
    /// `game` The game the player has joined.
    /// `slot` The slot in the game the player is in.
    pub fn set_game(&mut self, game: Option<GameID>) {
        self.game = game;
        self.update_client();
    }

    /// Updates the networking information for this session making
    /// it a set and setting the ext and groups. Updating the client
    /// with the new session details
    ///
    /// `groups` The networking groups
    /// `ext`    The networking ext
    pub fn set_network_info(&mut self, groups: NetGroups, ext: QosNetworkData) {
        let net = &mut &mut self.net;
        net.qos = ext;
        net.groups = Some(groups);
        self.update_client();
    }

    /// Updates the hardware flag for this session and
    /// updates the client with the changes
    ///
    /// `value` The new hardware flag value
    pub fn set_hardware_flag(&mut self, value: u16) {
        self.net.hardware_flags = value;
        self.update_client();
    }

    /// Updates the data stored on the client so that it matches
    /// the data stored in this session
    fn update_client(&mut self) {
        let player_id = self.player.as_ref().map(|player| player.id).unwrap_or(1);
        let packet = Packet::notify(
            Components::UserSessions(UserSessions::SetSession),
            SetSession {
                player_id,
                session: self,
            },
        );
        self.push(packet);
    }

    pub fn push_details(&mut self, addr: SessionAddr) {
        let player = match self.player.as_ref() {
            Some(value) => value,
            None => return,
        };

        // Create the details packets
        let a = Packet::notify(
            Components::UserSessions(UserSessions::SessionDetails),
            SessionUpdate {
                session: self,
                player_id: player.id,
                display_name: &player.display_name,
            },
        );

        let b = Packet::notify(
            Components::UserSessions(UserSessions::UpdateExtendedDataAttribute),
            UpdateExtDataAttr {
                flags: 0x3,
                player_id: player.id,
            },
        );

        // Push the packets
        addr.push(a);
        addr.push(b);
    }

    /// Removes the session from any connected games and the
    /// matchmaking queue
    pub fn remove_games(&mut self) {
        let game = self.game.take();
        let services = GlobalState::services();
        if let Some(game_id) = game {
            services
                .game_manager
                .remove_player(game_id, RemovePlayerType::Session(self.id))
        } else {
            services.matchmaking.unqueue_session(self.id);
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.remove_games();
        debug!("Session dropped (SID: {})", self.id);
    }
}

/// Structure for wrapping session details around a debug
/// packet message for logging
struct SessionPacketDebug<'a> {
    action: &'static str,
    packet: &'a Packet,
    component: Components,
    session: &'a Session,
}

impl Debug for SessionPacketDebug<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Session {} Packet", self.action)?;

        let component = &self.component;

        if let Some(player) = &self.session.player {
            writeln!(
                f,
                "Info: (Name: {}, ID: {}, SID: {})",
                &player.display_name, &player.id, &self.session.id
            )?;
        } else {
            writeln!(f, "Info: ( SID: {})", &self.session.id)?;
        }

        let minified = matches!(
            component,
            Components::Authentication(components::Authentication::ListUserEntitlements2)
                | Components::Util(components::Util::FetchClientConfig)
                | Components::Util(components::Util::UserSettingsLoadAll)
        );

        PacketDebug {
            packet: self.packet,
            component,
            minified,
        }
        .fmt(f)
    }
}

/// Encodes the session details for the provided session using
/// the provided writer
///
/// `session` The session to encode
/// `writer`  The writer to encode with
fn encode_session(session: &Session, writer: &mut TdfWriter) {
    session.net.tag_groups(b"ADDR", writer);
    writer.tag_str(b"BPS", "ea-sjc");
    writer.tag_str_empty(b"CTY");
    writer.tag_var_int_list_empty(b"CVAR");
    {
        writer.tag_map_start(b"DMAP", TdfType::VarInt, TdfType::VarInt, 1);
        writer.write_u32(0x70001);
        writer.write_u16(0x409a);
    }
    writer.tag_u16(b"HWFG", session.net.hardware_flags);
    {
        // Ping latency to the Quality of service servers
        writer.tag_list_start(b"PSLM", TdfType::VarInt, 1);
        0xfff0fff.encode(writer);
    }
    writer.tag_value(b"QDAT", &session.net.qos);
    writer.tag_u8(b"UATT", 0);
    if let Some(game_id) = &session.game {
        writer.tag_list_start(b"ULST", TdfType::Triple, 1);
        (4, 1, *game_id).encode(writer);
    }
    writer.tag_group_end();
}

/// Session update for a session other than ourselves
/// which contains the details for that session
struct SessionUpdate<'a> {
    /// The session this update is for
    session: &'a Session,
    /// The player ID the update is for
    player_id: PlayerID,
    /// The display name of the player the update is
    display_name: &'a str,
}

impl Encodable for SessionUpdate<'_> {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_group(b"DATA");
        encode_session(self.session, writer);

        writer.tag_group(b"USER");
        writer.tag_u32(b"AID", self.player_id);
        writer.tag_u32(b"ALOC", 0x64654445);
        writer.tag_empty_blob(b"EXBB");
        writer.tag_u8(b"EXID", 0);
        writer.tag_u32(b"ID", self.player_id);
        writer.tag_str(b"NAME", self.display_name);
        writer.tag_group_end();
    }
}

/// Session update for ourselves
struct SetSession<'a> {
    /// The player ID the update is for
    player_id: PlayerID,
    /// The session this update is for
    session: &'a Session,
}

impl Encodable for SetSession<'_> {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_group(b"DATA");
        encode_session(self.session, writer);
        writer.tag_u32(b"USID", self.player_id);
    }
}
