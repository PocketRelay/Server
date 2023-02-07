//! Sessions are client connections to the main server with associated
//! data such as player data for when they become authenticated and
//! networking data.
use super::models::errors::{ServerError, ServerResult};
use crate::utils::types::PlayerID;
use crate::{
    services::game::{player::GamePlayer, RemovePlayerType},
    state::GlobalState,
    utils::{
        components::{self, Components, UserSessions},
        models::{NetData, NetGroups, QosNetworkData, UpdateExtDataAttr},
        packet::append_packet_decoded,
        types::{GameID, SessionID},
    },
};
use blaze_pk::{codec::Encodable, tag::TdfType, writer::TdfWriter};
use blaze_pk::{
    packet::{Packet, PacketComponents, PacketType},
    router::{Router, State},
};
use database::Player;
use log::{debug, error, log_enabled};
use std::{collections::VecDeque, io, net::SocketAddr, sync::Arc};
use tokio::{
    net::TcpStream,
    select,
    sync::{mpsc, oneshot},
};

/// Structure for storing a client session. This includes the
/// network stream for the client along with global state and
/// other session state.
pub struct Session {
    /// Unique identifier for this session.
    pub id: SessionID,

    /// Underlying connection stream to client
    stream: TcpStream,

    /// The socket connection address of the client
    socket_addr: SocketAddr,

    /// If the session is authenticated it will have a linked
    /// player model from the database
    player: Option<Player>,

    /// Networking information
    net: NetData,

    /// The id of the game if connected to one
    game: Option<GameID>,

    /// The queue of packets that need to be written
    queue: VecDeque<Packet>,

    /// State determining whether the session has a flush message
    /// already queued in the reciever
    flush_queued: bool,

    /// Arc to router to use for routing
    router: Arc<Router<Components, SessionAddr>>,

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
    sender: mpsc::UnboundedSender<Message>,
}

impl SessionAddr {
    /// Writes a new packet ot the session
    ///
    /// `packet` The packet to write
    pub fn push(&self, packet: Packet) {
        self.sender.send(Message::Write(packet)).ok();
    }

    /// Sets the game that the session is apart of
    ///
    /// `game` The game
    pub fn set_game(&self, game: Option<GameID>) {
        self.sender.send(Message::SetGame(game)).ok();
    }

    pub fn push_details(&self) {
        self.sender.send(Message::PushDetails).ok();
    }
    pub fn clear_player(&self) {
        self.sender.send(Message::ClearPlayer).ok();
    }
    pub fn remove_games(&self) {
        self.sender.send(Message::RemoveGames).ok();
    }

    pub async fn try_into_player(&self) -> Option<GamePlayer> {
        let (tx, rx) = oneshot::channel();
        if let Err(_) = self.sender.send(Message::TryIntoPlayer(tx)) {
            return None;
        }

        rx.await.ok().flatten()
    }

    pub fn set_network_info(&self, groups: NetGroups, ext: QosNetworkData) {
        self.sender
            .send(Message::SetNetworkInfo { groups, ext })
            .ok();
    }
    pub fn set_hardware_flag(&self, value: u16) {
        self.sender.send(Message::SetHardwareFlag(value)).ok();
    }

    pub async fn set_player(&self, player: Player) -> ServerResult<(Player, String)> {
        let (tx, rx) = oneshot::channel();
        if let Err(_) = self.sender.send(Message::SetPlayer { player, tx }) {
            return Err(ServerError::ServerUnavailable);
        }

        rx.await.map_err(|_| ServerError::ServerUnavailable)?
    }

    pub async fn get_player(&self) -> ServerResult<Option<Player>> {
        let (tx, rx) = oneshot::channel();
        if let Err(_) = self.sender.send(Message::GetPlayer(tx)) {
            return Err(ServerError::ServerUnavailable);
        }

        rx.await.map_err(|_| ServerError::ServerUnavailable)
    }
    pub async fn get_player_id(&self) -> Option<u32> {
        let (tx, rx) = oneshot::channel();
        if let Err(_) = self.sender.send(Message::GetPlayerId(tx)) {
            return None;
        }

        rx.await.ok().flatten()
    }

    pub async fn socket_string(&self) -> Option<String> {
        let (tx, rx) = oneshot::channel();
        if let Err(_) = self.sender.send(Message::SocketString(tx)) {
            return None;
        }

        rx.await.ok()
    }

    pub fn spawn(
        id: SessionID,
        values: (TcpStream, SocketAddr),
        router: Arc<Router<Components, SessionAddr>>,
    ) {
        let (sender, receiver) = mpsc::unbounded_channel();
        let session = Session::new(id, values.0, values.1, sender, router);
        tokio::spawn(session.process(receiver));
    }
}

/// Message for communicating with the spawned session
/// using cloned the sender present on cloned addresses
enum Message {
    /// Changes the active game value
    SetGame(Option<GameID>),

    /// Writes a new packet to the outbound queue
    Write(Packet),

    /// Flushes the outbound queue
    Flush,

    TryIntoPlayer(oneshot::Sender<Option<GamePlayer>>),

    SetNetworkInfo {
        groups: NetGroups,
        ext: QosNetworkData,
    },

    SetHardwareFlag(u16),

    PushDetails,

    RemoveGames,

    GetPlayer(oneshot::Sender<Option<Player>>),
    GetPlayerId(oneshot::Sender<Option<u32>>),

    SetPlayer {
        player: Player,
        tx: oneshot::Sender<ServerResult<(Player, String)>>,
    },

    ClearPlayer,

    SocketString(oneshot::Sender<String>),
}

impl Session {
    /// Creates a new session with the provided values.
    ///
    /// `id`             The unique session ID
    /// `values`         The networking TcpStream and address
    /// `message_sender` The message sender for session messages
    fn new(
        id: SessionID,
        stream: TcpStream,
        addr: SocketAddr,
        sender: mpsc::UnboundedSender<Message>,
        router: Arc<Router<Components, SessionAddr>>,
    ) -> Self {
        Self {
            id,
            stream,
            socket_addr: addr,
            queue: VecDeque::new(),
            player: None,
            net: NetData::default(),
            game: None,
            flush_queued: false,
            router,
            addr: SessionAddr { id, sender },
        }
    }

    /// Processing function which handles recieving messages, flush notifications,
    /// reading packets, and handling safe shutdowns for this session. This function
    /// owns the session.
    ///
    /// `message` The receiver for receiving session messages
    async fn process(mut self, mut receiver: mpsc::UnboundedReceiver<Message>) {
        loop {
            select! {
                // Recieve session instruction messages
                message = receiver.recv() => {
                    if let Some(message) = message {
                        self.handle_message(message).await;
                    }
                }
                // Handle packet reads
                result = self.read() => {
                    if result.is_err() {
                        break;
                    }
                }
            };
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
        let router = self.router.clone();
        let mut addr = self.addr.clone();
        tokio::spawn(async move {
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

    /// Handles a message recieved for the session
    ///
    /// `message` The message that was recieved
    async fn handle_message(&mut self, message: Message) {
        match message {
            Message::SetGame(game) => self.set_game(game),
            Message::Write(packet) => self.push(packet),
            Message::Flush => self.flush().await,
            Message::TryIntoPlayer(tx) => {
                let player = self.try_into_player();
                tx.send(player).ok();
            }
            Message::SetNetworkInfo { groups, ext } => self.set_network_info(groups, ext),
            Message::SetHardwareFlag(value) => self.set_hardware_flag(value),
            Message::PushDetails => self.push_details(),
            Message::RemoveGames => self.remove_games(),
            Message::SetPlayer { player, tx } => {
                let result = self.set_player(player);
                tx.send(result).ok();
            }
            Message::GetPlayer(tx) => {
                let player = self.player.clone();
                tx.send(player).ok();
            }
            Message::SocketString(tx) => {
                let value = self.socket_addr.to_string();
                tx.send(value).ok();
            }
            Message::GetPlayerId(tx) => {
                let player = self.player.as_ref().map(|value| value.id);
                tx.send(player).ok();
            }
            Message::ClearPlayer => {
                self.player = None;
            }
        }
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
    fn debug_log_packet(&self, action: &str, packet: &Packet) {
        // Skip if debug logging is disabled
        if !log_enabled!(log::Level::Debug) {
            return;
        }

        let header = &packet.header;
        let component = Components::from_header(header);
        if Self::is_debug_ignored(&component) {
            return;
        }

        let mut message = String::new();
        message.push_str("\nSession ");
        message.push_str(action);
        message.push_str(" Packet");

        {
            message.push_str("\nInfo: (");

            if let Some(player) = self.player.as_ref() {
                message.push_str("Name: ");
                message.push_str(&player.display_name);
                message.push_str(", ID: ");
                message.push_str(&player.id.to_string());
                message.push_str(", SID: ");
                message.push_str(&self.id.to_string());
            } else {
                message.push_str("SID: ");
                message.push_str(&self.id.to_string());
            }

            message.push(')');
        }

        message.push_str(&format!("\nComponent: {:?}", component));
        message.push_str(&format!("\nType: {:?}", header.ty));
        if header.ty != PacketType::Notify {
            message.push_str("\nID: ");
            message.push_str(&header.id.to_string());
        }

        if header.ty == PacketType::Error {
            message.push_str("\nERROR: ");
            message.push_str(&header.error.to_string());
        }

        if !Self::is_debug_minified(&component) {
            append_packet_decoded(packet, &mut message);
        }

        debug!("{}", message);
    }

    /// Checks whether the provided `component` is ignored completely
    /// when debug logging. This is for packets such as Ping and SuspendUserPing
    /// where they occur frequently but provide no useful data for debugging.
    ///
    /// `component` The component to check
    fn is_debug_ignored(component: &Components) -> bool {
        Components::Util(components::Util::Ping).eq(component)
            || Components::Util(components::Util::SuspendUserPing).eq(component)
    }

    /// Checks whether the provided `component` should have its contents
    /// hidden when being debug printed. Used to hide the contents of
    /// larger packets.
    ///
    /// `component` The component to check
    fn is_debug_minified(component: &Components) -> bool {
        Components::Authentication(components::Authentication::ListUserEntitlements2).eq(component)
            || Components::Util(components::Util::FetchClientConfig).eq(component)
            || Components::Util(components::Util::UserSettingsLoadAll).eq(component)
    }

    /// Queues a new flush if there is not already one queued
    fn queue_flush(&mut self) {
        if !self.flush_queued {
            self.flush_queued = true;
            self.addr.sender.send(Message::Flush).ok();
        }
    }

    /// Flushes the output buffer
    async fn flush(&mut self) {
        self.flush_queued = false;

        // Counter for the number of items written
        let mut write_count = 0usize;
        while let Some(item) = self.queue.pop_front() {
            self.debug_log_packet("Wrote", &item);
            match item.write_async(&mut self.stream).await {
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

    /// Reads a packet from the stream and then passes the packet
    /// onto `handle_packet` awaiting the result of that
    async fn read(&mut self) -> io::Result<()> {
        let packet: Packet = Packet::read_async(&mut self.stream).await?;
        self.handle_packet(packet);
        Ok(())
    }

    /// Sets the game details for the current session and updates
    /// the client with the new sesion details
    ///
    /// `game` The game the player has joined.
    /// `slot` The slot in the game the player is in.
    fn set_game(&mut self, game: Option<GameID>) {
        self.game = game;
        self.update_client();
    }

    /// Updates the networking information for this session making
    /// it a set and setting the ext and groups. Updating the client
    /// with the new session details
    ///
    /// `groups` The networking groups
    /// `ext`    The networking ext
    fn set_network_info(&mut self, groups: NetGroups, ext: QosNetworkData) {
        let net = &mut &mut self.net;
        net.qos = ext;
        net.groups = Some(groups);
        self.update_client();
    }

    /// Updates the hardware flag for this session and
    /// updates the client with the changes
    ///
    /// `value` The new hardware flag value
    fn set_hardware_flag(&mut self, value: u16) {
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

    fn set_player(&mut self, player: Player) -> ServerResult<(Player, String)> {
        // Update the player value
        let player = self.player.insert(player);
        let player = player.clone();

        let services = GlobalState::services();

        let token = match services.jwt.claim(&player) {
            Ok(value) => value,
            Err(err) => {
                error!("Unable to create session token for player: {:?}", err);
                return Err(ServerError::ServerUnavailable);
            }
        };

        Ok((player, token))
    }

    fn push_details(&mut self) {
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
        self.push(a);
        self.push(b);
    }

    /// Removes the session from any connected games and the
    /// matchmaking queue
    fn remove_games(&mut self) {
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
pub struct SessionUpdate<'a> {
    /// The session this update is for
    pub session: &'a Session,
    /// The player ID the update is for
    pub player_id: PlayerID,
    /// The display name of the player the update is
    pub display_name: &'a str,
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
pub struct SetSession<'a> {
    /// The player ID the update is for
    pub player_id: PlayerID,
    /// The session this update is for
    pub session: &'a Session,
}

impl Encodable for SetSession<'_> {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_group(b"DATA");
        encode_session(self.session, writer);
        writer.tag_u32(b"USID", self.player_id);
    }
}
