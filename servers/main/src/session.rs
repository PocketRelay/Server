//! This module contains the storage and additional data for sessions. Sessions
//! are data attached to streams that can be manipulated. Sessions are stored
//! behind Arc's and are cloned into Games and other resources. Sesssion must be
//! removed from all other structs in the release function.

use std::{
    collections::VecDeque,
    io,
    net::{IpAddr, SocketAddr},
};

use core::{
    game::player::{GamePlayer, SessionMessage},
    state::GlobalState,
};

use database::players;
use utils::{
    net::public_address,
    types::{GameID, PlayerID, SessionID},
};

use blaze_pk::{
    codec::{Codec, Reader},
    packet::{Packet, PacketComponents},
    tag::Tag,
};

use log::{debug, error, log_enabled};
use tokio::{
    net::TcpStream,
    select,
    sync::{mpsc, Mutex, Notify},
};

use core::blaze::{
    codec::{NetAddress, NetData, NetGroups, QosNetworkData, UpdateExtDataAttr},
    components::{self, Components, UserSessions},
    errors::HandleResult,
};

use crate::{
    codec::{SessionUpdate, SetSession},
    routes,
};

/// Structure for storing a client session. This includes the
/// network stream for the client along with global state and
/// other session state.
pub struct Session {
    /// Unique identifier for this session.
    pub id: SessionID,

    /// Underlying connection stream to client
    pub stream: Mutex<TcpStream>,
    /// The socket connection address of the client
    pub addr: SocketAddr,

    /// If the session is authenticated it will have a linked
    /// player model from the database
    pub player: Option<players::Model>,

    /// Networking information
    pub net: NetData,

    /// The id of the game if connected to one
    pub game: Option<GameID>,

    /// The queue of packets that need to be written
    queue: VecDeque<Packet>,
    /// Sender for flushing packets
    flush: Notify,
    /// Sender for session messages
    message_sender: mpsc::Sender<SessionMessage>,
}

impl Session {
    pub fn into_player(&self) -> Option<GamePlayer> {
        let player = self.player.as_ref()?;
        Some(GamePlayer::new(
            self.id,
            player.id,
            player.display_name.clone(),
            self.net,
            self.message_sender.clone(),
        ))
    }

    pub fn spawn(id: SessionID, values: (TcpStream, SocketAddr)) {
        let (message_sender, message_recv) = mpsc::channel(20);
        let session = Self {
            id,
            stream: Mutex::new(values.0),
            addr: values.1,
            queue: VecDeque::new(),
            flush: Notify::new(),
            message_sender,
            player: None,
            net: NetData::default(),
            game: None,
        };
        tokio::spawn(session.process(message_recv));
    }

    async fn process(mut self, mut message: mpsc::Receiver<SessionMessage>) {
        let mut shutdown = GlobalState::shutdown();
        loop {
            select! {
                message = message.recv() => {
                    if let Some(message) = message {
                        self.handle_message(message).await;
                    }
                }
                _ = self.flush.notified() => { self.flush().await; }
                result = self.read() => {
                    if let Ok((component, packet)) = result {
                        self.handle_packet(component, &packet).await;
                    } else {
                        break;
                    }
                }
                _ = shutdown.changed() => {break;}
            };
        }
    }

    /// Handles processing a recieved packet from the `process` function. This includes a
    /// component for routing and the actual packet itself. The buffer is flushed after
    /// routing is complete.
    ///
    /// `session`   The session to process the packet for
    /// `component` The component of the packet for routing
    /// `packet`    The packet itself
    async fn handle_packet(&mut self, component: Components, packet: &Packet) {
        Session::debug_log_packet(self, "Read", packet);
        if let Err(err) = routes::route(self, component, packet).await {
            error!("Error occurred while routing (SID: {}): {:?}", self.id, err);
        }
        self.flush().await;
    }

    pub async fn handle_message(&mut self, message: SessionMessage) {
        match message {
            SessionMessage::SetGame(game) => self.set_game(game),
            SessionMessage::Packet(packet) => self.push(packet),
            SessionMessage::Packets(packets) => self.push_all(packets),
        }
    }

    /// Pushes a new packet to the back of the packet buffer
    /// and sends a flush notification
    ///
    /// `packet` The packet to push to the buffer
    pub fn push(&mut self, packet: Packet) {
        self.queue.push_back(packet);
        self.flush.notify_one();
    }

    /// Pushes all the provided packets to the packet buffer
    /// and sends a flush notification after all the packets
    /// are pushed.
    ///
    /// `packets` The packets to push to the buffer
    pub fn push_all(&mut self, packets: Vec<Packet>) {
        self.queue.reserve(packets.len());
        for packet in packets {
            self.queue.push_back(packet);
        }
        self.flush.notify_one();
    }

    /// Logs the contents of the provided packet to the debug output along with
    /// the header information and basic session information.
    ///
    /// `action` The name of the action this packet is undergoing.
    ///          (e.g. Writing or Reading)
    /// `packet` The packet that is being logged
    pub fn debug_log_packet(&self, action: &str, packet: &Packet) {
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
        message.push_str(&format!("\nID: {}", header.id));

        if Self::is_debug_minified(&component) {
            debug!("{}", message);
            return;
        }

        let mut reader = Reader::new(&packet.contents);
        let mut out = String::new();
        out.push_str("{\n");
        match Tag::stringify(&mut reader, &mut out, 1) {
            Ok(_) => {}
            Err(err) => {
                message.push_str("\nExtra: Content was malformed");
                message.push_str(&format!("\nError: {:?}", err));
                message.push_str(&format!("\nPartial Content: {}", out));
                debug!("{}", message);
                return;
            }
        };
        if out.len() == 2 {
            // Remove new line if nothing else was appended
            out.pop();
        }
        out.push('}');
        message.push_str(&format!("\nContent: {}", out));
        debug!("{}", message);
    }

    /// Checks whether the provided `component` is ignored completely
    /// when debug logging. This is for packets such as Ping and SuspendUserPing
    /// where they occur frequently but provide no useful data for debugging.
    fn is_debug_ignored(component: &Components) -> bool {
        Components::Util(components::Util::Ping).eq(component)
            || Components::Util(components::Util::SuspendUserPing).eq(component)
    }

    /// Checks whether the provided `component` should have its contents
    /// hidden when being debug printed. Used to hide the contents of
    /// larger packets.
    fn is_debug_minified(component: &Components) -> bool {
        Components::Authentication(components::Authentication::ListUserEntitlements2).eq(component)
            || Components::Util(components::Util::FetchClientConfig).eq(component)
            || Components::Util(components::Util::UserSettingsLoadAll).eq(component)
    }

    /// Flushes the output buffer
    pub async fn flush(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        // Counter for the number of items written
        let mut write_count = 0usize;

        let stream = &mut *self.stream.lock().await;
        while let Some(item) = self.queue.pop_front() {
            Self::debug_log_packet(self, "Wrote", &item);
            match item.write_async(stream).await {
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
    pub async fn write_immediate(&self, packet: &Packet) -> io::Result<()> {
        let stream = &mut *self.stream.lock().await;
        packet.write_async(stream).await?;
        self.debug_log_packet("Wrote", packet);
        Ok(())
    }

    /// Attempts to read a packet from the client stream.
    pub async fn read(&self) -> io::Result<(Components, Packet)> {
        let stream = &mut *self.stream.lock().await;
        Packet::read_async_typed(stream).await
    }

    /// Shortcut for response packets. These are written directly as they are
    /// only ever used client processing tasks.
    ///
    /// `packet`   The packet to respond to.
    /// `contents` The contents of the response packet.
    ///
    pub async fn response<T: Codec>(&self, packet: &Packet, contents: &T) -> HandleResult {
        let response = Packet::response(packet, contents);
        self.write_immediate(&response).await?;
        Ok(())
    }

    /// Shortcut for responses that have empty contents.
    ///
    /// `packet` The packet to respond to.
    pub async fn response_empty(&self, packet: &Packet) -> HandleResult {
        let response = Packet::response_empty(packet);
        self.write_immediate(&response).await?;
        Ok(())
    }

    /// Shortcut for error responses that have empty contents
    ///
    /// `packet` The packet to respond to.
    /// `error`  The error for the packet.
    pub async fn response_error(&self, packet: &Packet, error: impl Into<u16>) -> HandleResult {
        let response = Packet::error_empty(packet, error);
        self.write_immediate(&response).await?;
        Ok(())
    }

    /// Writes a new notify packet directly to the client stream
    ///
    /// `component` The component for the packet.
    /// `contents`  The contents of the packet.
    pub async fn notify_immediate<T: Codec>(
        &self,
        component: Components,
        contents: &T,
    ) -> HandleResult {
        let packet = Packet::notify(component, contents);
        self.write_immediate(&packet).await?;
        Ok(())
    }

    /// Retrieves the ID of the underlying player returning on failure
    /// will return 1 as a fallback value.
    pub fn player_id_safe(&self) -> PlayerID {
        self.player.as_ref().map(|player| player.id).unwrap_or(1)
    }

    /// Retrieves the ID of the underlying player returning None on failure
    pub fn player_id(&self) -> Option<PlayerID> {
        self.player.as_ref().map(|player| player.id)
    }

    /// Sets the player thats attached to this session. Will log information
    /// about the previous player if there was one
    ///
    /// `player` The player to set the state to or None to clear the player
    pub fn set_player(&mut self, player: players::Model) {
        let existing = self.player.replace(player);
        if let Some(existing) = existing {
            debug!(
                "Swapped authentication from:\nPrevious (ID: {}, Username: {}, Email: {})",
                existing.id, existing.display_name, existing.email,
            );
        }
    }

    /// Clears the current player value
    pub fn clear_player(&mut self) {
        self.player = None;
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
            return NetAddress(0);
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
    pub fn update_client(&mut self) {
        let packet = Packet::notify(
            Components::UserSessions(UserSessions::SetSession),
            &SetSession {
                player_id: self.player_id_safe(),
                session: self,
            },
        );
        self.push(packet);
    }

    pub fn update_self(&mut self) {
        let Some(player) = self.player.as_ref() else {return;};
        let packets = vec![
            Packet::notify(
                Components::UserSessions(UserSessions::SessionDetails),
                &SessionUpdate {
                    session: self,
                    player_id: player.id,
                    display_name: &player.display_name,
                },
            ),
            Packet::notify(
                Components::UserSessions(UserSessions::UpdateExtendedDataAttribute),
                &UpdateExtDataAttr {
                    flags: 0x3,
                    player_id: player.id,
                },
            ),
        ];
        self.push_all(packets);
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        debug!("Session dropped (SID: {})", self.id);
        let game = self.game.take();
        let session_id = self.id;

        tokio::spawn(async move {
            debug!("Cleaning up dropped session (SID: {})", session_id);
            let games = GlobalState::games();
            if let Some(game) = game {
                games.remove_player_sid(game, session_id).await;
            } else {
                games.unqueue_session(session_id).await;
            }
            debug!("Finished cleaning up dropped session (SID: {})", session_id)
        });
    }
}
