//! Sessions are client connections to the main server with associated
//! data such as player data for when they become authenticated and
//! networking data.
use super::{
    models::session::{SessionUpdate, SetSession},
    router::Router,
};
use crate::{
    blaze::{
        append_packet_decoded,
        codec::{NetData, NetGroups, QosNetworkData, UpdateExtDataAttr},
        components::{self, Components, UserSessions},
    },
    game::{player::GamePlayer, RemovePlayerType},
    state::GlobalState,
    utils::types::{GameID, SessionID},
};
use blaze_pk::packet::{Packet, PacketComponents, PacketType};
use database::Player;
use log::{debug, error, log_enabled};
use std::{collections::VecDeque, io, net::SocketAddr, sync::Arc};
use tokio::{net::TcpStream, select, sync::mpsc};

/// Structure for storing a client session. This includes the
/// network stream for the client along with global state and
/// other session state.
pub struct Session {
    /// Unique identifier for this session.
    pub id: SessionID,

    /// Underlying connection stream to client
    stream: TcpStream,

    /// The socket connection address of the client
    pub socket_addr: SocketAddr,

    /// If the session is authenticated it will have a linked
    /// player model from the database
    pub player: Option<Player>,

    /// Networking information
    pub net: NetData,

    /// The id of the game if connected to one
    pub game: Option<GameID>,

    /// The queue of packets that need to be written
    queue: VecDeque<Packet>,

    /// State determining whether the session has a flush message
    /// already queued in the reciever
    flush_queued: bool,

    router: Arc<Router>,

    /// Internal address used for routing can be cloned and used elsewhere
    addr: SessionAddr,
}

/// Address to a session which allows manipulating sessions asyncronously
/// using mpsc channels without actually having access to the session itself
#[derive(Clone)]
pub struct SessionAddr {
    /// The ID this session is linked to
    pub id: SessionID,
    /// The sender for sending message to this session
    sender: mpsc::UnboundedSender<SessionMessage>,
}

impl SessionAddr {
    /// Writes a new packet ot the session
    ///
    /// `packet` The packet to write
    pub fn push(&self, packet: Packet) {
        self.sender.send(SessionMessage::Write(packet)).ok();
    }

    /// Sets the game that the session is apart of
    ///
    /// `game` The game
    pub fn set_game(&self, game: Option<GameID>) {
        self.sender.send(SessionMessage::SetGame(game)).ok();
    }

    pub fn flush(&self) {
        self.sender.send(SessionMessage::Flush).ok();
    }
}

/// Enum of different messages that can be sent to this
/// session in order to change it in different ways
#[derive(Debug)]
pub enum SessionMessage {
    /// Changes the active game value
    SetGame(Option<GameID>),

    /// Writes a new packet to the outbound queue
    Write(Packet),

    /// Flushes the outbound queue
    Flush,
}

impl Session {
    pub fn spawn(id: SessionID, values: (TcpStream, SocketAddr), router: Arc<Router>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        let session = Session::new(id, values.0, values.1, sender, router);
        tokio::spawn(session.process(receiver));
    }

    /// Creates a new session with the provided values.
    ///
    /// `id`             The unique session ID
    /// `values`         The networking TcpStream and address
    /// `message_sender` The message sender for session messages
    fn new(
        id: SessionID,
        stream: TcpStream,
        addr: SocketAddr,
        sender: mpsc::UnboundedSender<SessionMessage>,
        router: Arc<Router>,
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
    async fn process(mut self, mut receiver: mpsc::UnboundedReceiver<SessionMessage>) {
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
    pub fn try_into_player(&self) -> Option<GamePlayer> {
        let player = self.player.clone()?;
        Some(GamePlayer::new(player, self.net.clone(), self.addr.clone()))
    }

    /// Handles processing a recieved packet from the `process` function. This includes a
    /// component for routing and the actual packet itself. The buffer is flushed after
    /// routing is complete.
    ///
    /// `session`   The session to process the packet for
    /// `component` The component of the packet for routing
    /// `packet`    The packet itself
    async fn handle_packet(&mut self, packet: Packet) -> io::Result<()> {
        self.debug_log_packet("Read", &packet);
        let router = self.router.clone();
        match router.handle(self, packet).await {
            Ok(packet) => {
                self.write(packet).await?;
            }
            Err(err) => {
                error!("Error occurred while decoding packet: {:?}", err);
            }
        }
        self.flush().await;
        Ok(())
    }

    /// Handles a message recieved for the session
    ///
    /// `message` The message that was recieved
    async fn handle_message(&mut self, message: SessionMessage) {
        match message {
            SessionMessage::SetGame(game) => self.set_game(game),
            SessionMessage::Write(packet) => self.push(packet),
            SessionMessage::Flush => self.flush().await,
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
    /// Writes the provided packet directly to the underlying stream
    /// rather than pushing to the buffer. Only use when handling
    /// responses will cause long blocks because will wait for all
    /// the data to be written.
    async fn write(&mut self, packet: Packet) -> io::Result<()> {
        packet.write_async(&mut self.stream).await?;
        self.debug_log_packet("Wrote", &packet);
        Ok(())
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
        if false || Self::is_debug_ignored(&component) {
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

        if true || !Self::is_debug_minified(&component) {
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
            self.addr.sender.send(SessionMessage::Flush).ok();
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
        self.handle_packet(packet).await
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
    pub fn set_network_info(&mut self, groups: NetGroups, ext: QosNetworkData) {
        let net = &mut &mut self.net;
        net.is_set = true;
        net.qos = ext;
        if net.groups.external.0.is_invalid() && !groups.external.0.is_invalid() {
            net.groups = groups;
        }
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

    pub fn update_self(&mut self) {
        let Some(player) = self.player.as_ref() else {return;};
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
        self.push(a);
        self.push(b);
    }

    /// Removes the session from any connected games and the
    /// matchmaking queue
    pub fn remove_games(&mut self) {
        let game = self.game.take();
        let games = GlobalState::games();
        if let Some(game_id) = game {
            games.remove_player(game_id, RemovePlayerType::Session(self.id));
        } else {
            games.unqueue_session(self.id);
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.remove_games();
        debug!("Session dropped (SID: {})", self.id);
    }
}
