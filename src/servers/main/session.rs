//! Sessions are client connections to the main server with associated
//! data such as player data for when they become authenticated and
//! networking data.
use super::{
    models::session::{SessionUpdate, SetSession},
    routes,
};
use crate::{
    blaze::{
        append_packet_decoded,
        codec::{NetAddress, NetData, NetGroups, QosNetworkData, UpdateExtDataAttr},
        components::{self, Components, UserSessions},
        errors::{BlazeError, ServerError},
    },
    game::{player::GamePlayer, RemovePlayerType},
    state::GlobalState,
    utils::{
        net::public_address,
        types::{GameID, SessionID},
    },
};
use blaze_pk::packet::{Packet, PacketComponents, PacketType};
use database::Player;
use log::{debug, error, log_enabled};
use std::{
    collections::VecDeque,
    io,
    net::{IpAddr, SocketAddr},
};
use tokio::{net::TcpStream, select, sync::mpsc};

/// Structure for storing a client session. This includes the
/// network stream for the client along with global state and
/// other session state.
pub struct Session {
    /// Unique identifier for this session.
    pub id: SessionID,

    /// Underlying connection stream to client
    pub stream: TcpStream,

    /// The socket connection address of the client
    pub addr: SocketAddr,

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

    /// Sender for session messages
    sender: mpsc::UnboundedSender<SessionMessage>,
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
    pub fn spawn(id: SessionID, values: (TcpStream, SocketAddr)) {
        let (sender, receiver) = mpsc::unbounded_channel();
        let session = Session::new(id, values.0, values.1, sender);
        tokio::spawn(session.process(receiver));
    }

    /// Creates a new session with the provided values.
    ///
    /// `id`             The unique session ID
    /// `values`         The networking TcpStream and address
    /// `message_sender` The message sender for session messages
    pub fn new(
        id: SessionID,
        stream: TcpStream,
        addr: SocketAddr,
        sender: mpsc::UnboundedSender<SessionMessage>,
    ) -> Self {
        Self {
            id,
            stream,
            addr,
            queue: VecDeque::new(),
            sender,
            player: None,
            net: NetData::default(),
            game: None,
            flush_queued: false,
        }
    }

    /// Processing function which handles recieving messages, flush notifications,
    /// reading packets, and handling safe shutdowns for this session. This function
    /// owns the session.
    ///
    /// `message` The receiver for receiving session messages
    pub async fn process(mut self, mut receiver: mpsc::UnboundedReceiver<SessionMessage>) {
        let mut shutdown = GlobalState::shutdown();
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
                // Shutdown hook to ensure we don't keep trying to process after shutdown
                _ = shutdown.changed() => { break; }
            };
        }
        self.release().await;
    }

    /// Handles processing a recieved packet from the `process` function. This includes a
    /// component for routing and the actual packet itself. The buffer is flushed after
    /// routing is complete.
    ///
    /// `session`   The session to process the packet for
    /// `component` The component of the packet for routing
    /// `packet`    The packet itself
    async fn handle_packet(&mut self, component: Components, packet: &Packet) {
        self.debug_log_packet("Read", packet);
        match routes::route(self, component, packet).await {
            Ok(response) => {
                if let Err(err) = self.write(response).await {
                    error!(
                        "Error occurred while responding (SID: {}): {:?}",
                        self.id, err
                    );
                }
            }
            Err(err) => {
                let error = if let BlazeError::Server(err) = err {
                    err
                } else {
                    error!("Error occurred while routing (SID: {}): {:?}", self.id, err);
                    ServerError::ServerUnavailable
                };
                let response = Packet::error_empty(packet, error as u16);
                self.write(response).await.ok();
            }
        }
        self.flush().await;
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
            message.push_str(&format!("\nID: {}", header.id));
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
    pub fn queue_flush(&mut self) {
        if !self.flush_queued {
            self.flush_queued = true;
            self.sender.send(SessionMessage::Flush).ok();
        }
    }

    /// Flushes the output buffer
    pub async fn flush(&mut self) {
        self.flush_queued = false;
        if self.queue.is_empty() {
            return;
        }
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
        debug!("Flushed session (SID: {}, Count: {})", self.id, write_count);
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

    /// Reads a packet from the stream and then passes the packet
    /// onto `handle_packet` awaiting the result of that
    async fn read(&mut self) -> io::Result<()> {
        let (component, packet): (Components, Packet) =
            Packet::read_async_typed(&mut self.stream).await?;
        self.handle_packet(component, &packet).await;
        Ok(())
    }

    /// Sets the player thats attached to this session. Will log information
    /// about the previous player if there was one. Returns a mutable reference
    /// to the player that was inserted
    ///
    /// `player` The player to set the state to or None to clear the player
    pub fn set_player(&mut self, player: Player) -> &mut Player {
        if let Some(existing) = self.player.take() {
            debug!(
                "Swapped authentication from:\nPrevious (ID: {}, Username: {}, Email: {})",
                existing.id, existing.display_name, existing.email,
            );
        }
        self.player.insert(player)
    }

    /// Clears the current player value
    pub fn clear_player(&mut self) {
        self.player = None;
    }

    /// Attempts to obtain a game player from this session will return None
    /// if this session is not authenticated
    pub fn try_into_player(&self) -> Option<GamePlayer> {
        let player = self.player.as_ref()?;
        Some(GamePlayer::new(
            self.id,
            player.id,
            player.display_name.clone(),
            self.net,
            self.sender.clone(),
        ))
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
    pub async fn set_network_info(&mut self, groups: NetGroups, ext: QosNetworkData) {
        let net = &mut &mut self.net;
        net.is_set = true;
        net.qos = ext;
        net.groups = groups;
        self.update_missing_external().await;
        self.update_client();
    }

    /// Updates the external address field if its invalid or missing
    /// on the provided network group. Uses the session stored
    /// address information.
    ///
    /// `groups` The groups to modify
    async fn update_missing_external(&mut self) {
        let groups = &mut self.net.groups;
        let external = &mut groups.external;
        if external.0.is_invalid() || external.1 == 0 {
            // Match port with internal address
            external.1 = groups.internal.1;
            external.0 = Self::get_network_address(&self.addr).await;
        }
    }

    /// Obtains the networking address from the provided SocketAddr
    /// if the address is a loopback or private address then the
    /// public IP address of the network is used instead.
    ///
    /// `value` The socket address
    async fn get_network_address(addr: &SocketAddr) -> NetAddress {
        let ip = addr.ip();
        if let IpAddr::V4(value) = ip {
            // Value is local or private
            if value.is_loopback() || value.is_private() {
                if let Some(public_addr) = public_address().await {
                    return NetAddress::from_ipv4(&public_addr);
                }
            }
            let value = format!("{}", value);
            NetAddress::from_ipv4(&value)
        } else {
            // Don't know how to handle IPv6 addresses
            NetAddress(0)
        }
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

    pub async fn release(&mut self) {
        self.remove_games().await;
        debug!("Finished releasing up session (SID: {})", self.id)
    }

    /// Removes the session from any connected games and the
    /// matchmaking queue
    pub async fn remove_games(&mut self) {
        let game = self.game.take();
        let games = GlobalState::games();
        if let Some(game_id) = game {
            games
                .remove_player(game_id, RemovePlayerType::Session(self.id))
                .await;
        } else {
            games.unqueue_session(self.id).await;
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        debug!("Session dropped (SID: {})", self.id);
    }
}
