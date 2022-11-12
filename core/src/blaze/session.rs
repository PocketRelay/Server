//! This module contains the storage and additional data for sessions. Sessions
//! are data attached to streams that can be manipulated. Sessions are stored
//! behind Arc's and are cloned into Games and other resources. Sesssion must be
//! removed from all other structs in the release function.

use std::{collections::VecDeque, io, net::SocketAddr, sync::Arc};

use crate::{
    blaze::errors::BlazeError,
    game::{matchmaking::Matchmaking, Games},
    retriever::Retriever,
    GlobalStateArc,
};

use database::{players, Database, PlayersInterface};
use utils::random::generate_random_string;

use blaze_pk::{
    Codec, OpaquePacket, PacketComponents, PacketResult, PacketType, Packets, Reader, Tag,
};
use log::{debug, error, info, log_enabled};
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    sync::{mpsc, Mutex, RwLock},
};

use super::{
    components::{self, Components, UserSessions},
    errors::{BlazeResult, HandleResult},
    shared::{NetData, SessionDetails, SetSessionDetails, UpdateExtDataAttr},
};

/// Structure for storing a client session. This includes the
/// network stream for the client along with global state and
/// other session state.
pub struct Session {
    /// Reference to the global state. In order to access
    /// the database and other shared functionality
    pub global: GlobalStateArc,

    /// Unique identifier for this session.
    pub id: u32,

    /// Underlying connection stream to client
    pub stream: Mutex<TcpStream>,
    /// The socket connection address of the client
    pub addr: SocketAddr,

    /// Additional data stored on this session.
    pub data: RwLock<SessionData>,

    /// Buffer for notify packets that need to be written
    /// and flushed.
    buffer: SessionBuffer,

    /// Extra information for this session to include in
    /// the debug messages.
    debug_state: RwLock<String>,
}

impl Session {
    /// Creates a new session from the provided values
    ///
    /// `global`     The global state
    /// `id`         The ID of the session
    /// `values`     The tcp stream and socket address tuple from listening
    /// `flush_send` The flush sender for sending flush commands
    pub fn new(
        global: GlobalStateArc,
        id: u32,
        values: (TcpStream, SocketAddr),
        flush_send: mpsc::Sender<()>,
    ) -> Arc<Self> {
        Arc::new(Self {
            global,
            id,
            stream: Mutex::new(values.0),
            addr: values.1,
            data: RwLock::new(SessionData::default()),
            buffer: SessionBuffer::new(flush_send),
            debug_state: RwLock::new(format!("ID: {}", id)),
        })
    }

    /// Logs the contents of the provided packet to the debug output along with
    /// the header information and basic session information.
    ///
    /// `action` The name of the action this packet is undergoing.
    ///          (e.g. Writing or Reading)
    /// `packet` The packet that is being logged
    pub async fn debug_log_packet(&self, action: &str, packet: &OpaquePacket) {
        // Skip if debug logging is disabled
        if !log_enabled!(log::Level::Debug) {
            return;
        }

        let header = &packet.0;
        let component = Components::from_values(
            header.component,
            header.command,
            header.ty == PacketType::Notify,
        );

        if Self::is_debug_ignored(&component) {
            return;
        }

        let debug_info = &*self.debug_state.read().await;

        let mut message = String::new();
        message.push_str("\nSession ");
        message.push_str(action);
        message.push_str(" Packet");

        {
            message.push_str("\nInfo: (");
            message.push_str(debug_info);
            message.push(')');
        }

        message.push_str(&format!("\nComponent: {:?}", component));
        message.push_str(&format!("\nType: {:?}", header.ty));
        message.push_str(&format!("\nID: {}", header.id));

        if Self::is_debug_minified(&component) {
            debug!("{}", message);
            return;
        }

        let mut reader = Reader::new(&packet.1);
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
    pub async fn flush(&self) {
        self.buffer.flush(self).await;
    }

    /// Writes the provided packet to the underlying buffer to be
    /// flushed later.
    pub async fn write(&self, packet: &OpaquePacket) {
        self.debug_log_packet("Queued Write", packet).await;
        self.buffer.write(packet).await;
    }

    /// Writes all the provided packets to the underlying buffer to
    /// be flushed later.
    pub async fn write_all(&self, packets: &Vec<OpaquePacket>) {
        for packet in packets {
            self.debug_log_packet("Queued Write", packet).await;
        }
        self.buffer.write_all(packets).await;
    }

    /// Writes the provided packet directly to the underlying stream
    /// rather than pushing to the buffer. Only use when handling
    /// responses will cause long blocks because will wait for all
    /// the data to be written.
    pub async fn write_immediate(&self, packet: &OpaquePacket) -> io::Result<()> {
        let stream = &mut *self.stream.lock().await;
        packet.write_async(stream).await?;
        self.debug_log_packet("Wrote", packet).await;
        Ok(())
    }

    /// Writes all the provided packets directly to the underlying stream
    /// rather than pushing to the buffer. Only use when handling
    /// responses will cause long blocks because will wait for all
    /// the data to be written.
    pub async fn write_all_immediate(&self, packets: &Vec<OpaquePacket>) -> io::Result<()> {
        let stream = &mut *self.stream.lock().await;
        for packet in packets {
            packet.write_async(stream).await?;
            self.debug_log_packet("Wrote", packet).await;
        }
        Ok(())
    }

    /// Attempts to read a packet from the client stream.
    pub async fn read(&self) -> PacketResult<(Components, OpaquePacket)> {
        let stream = &mut *self.stream.lock().await;
        OpaquePacket::read_async_typed(stream).await
    }

    /// Shortcut for response packets. These are written directly as they are
    /// only ever used client processing tasks.
    ///
    /// `packet`   The packet to respond to.
    /// `contents` The contents of the response packet.
    ///
    pub async fn response<T: Codec>(&self, packet: &OpaquePacket, contents: &T) -> HandleResult {
        let response = Packets::response(packet, contents);
        self.write_immediate(&response).await?;
        Ok(())
    }

    /// Shortcut for responses that have empty contents.
    ///
    /// `packet` The packet to respond to.
    pub async fn response_empty(&self, packet: &OpaquePacket) -> HandleResult {
        let response = Packets::response_empty(packet);
        self.write_immediate(&response).await?;
        Ok(())
    }

    /// Shortcut for error response packets. These are written directly as they are
    /// only ever used client processing tasks.
    ///
    /// `packet`   The packet to respond to.
    /// `error`    The error for the packet.
    /// `contents` The contents of the response packet.
    pub async fn response_error<T: Codec>(
        &self,
        packet: &OpaquePacket,
        error: impl Into<u16>,
        contents: &T,
    ) -> HandleResult {
        let response = Packets::error(packet, error, contents);
        self.write_immediate(&response).await?;
        Ok(())
    }

    /// Shortcut for error responses that have empty contents
    ///
    /// `packet` The packet to respond to.
    /// `error`  The error for the packet.
    pub async fn response_error_empty(
        &self,
        packet: &OpaquePacket,
        error: impl Into<u16>,
    ) -> HandleResult {
        let response = Packets::error_empty(packet, error);
        self.write_immediate(&response).await?;
        Ok(())
    }

    /// Writes a new notify packet to the outbound buffer.
    ///
    /// `component` The component for the packet.
    /// `contents`  The contents of the packet.
    pub async fn notify<T: Codec>(&self, component: Components, contents: &T) {
        let packet = Packets::notify(component, contents);
        self.write(&packet).await;
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
        let packet = Packets::notify(component, contents);
        self.write_immediate(&packet).await?;
        Ok(())
    }

    /// Function for retrieving a reference to the database
    /// stored on the global state attached to this session
    pub fn db(&self) -> &Database {
        &self.global.db
    }

    /// Function for retrieving a reference to the retriever
    /// stored on the global state if one is present
    pub fn retriever(&self) -> Option<&Retriever> {
        self.global.retriever.as_ref()
    }

    /// Function for retrieving a reference to the games
    /// manager stored on the global state attached to this session
    pub fn games(&self) -> &Games {
        &self.global.games
    }

    /// Function for retrieving a reference to the matchmaking
    /// manager stored on the global state attached to this session
    pub fn matchmaking(&self) -> &Matchmaking {
        &self.global.matchmaking
    }

    /// Retrieves the ID of the underlying player returning on failure
    /// will return 1 as a fallback value.
    pub async fn player_id_safe(&self) -> u32 {
        let session_data = self.data.read().await;
        session_data.id_safe()
    }

    /// Attempts to retrieve the ID of the underlying player
    /// will return None if there is no player
    pub async fn player_id(&self) -> Option<u32> {
        let session_data = self.data.read().await;
        session_data.player.as_ref().map(|player| player.id)
    }

    /// Sets the debug state value to the provided value
    ///
    /// `value` The new debug state value.
    async fn set_debug_state(&self, value: String) {
        let state = &mut *self.debug_state.write().await;
        state.clear();
        state.push_str(&value);
    }

    /// Sets the player thats attached to this session. Will log information
    /// about the previous player if there was one
    ///
    /// `player` The player to set the state to or None to clear the player
    pub async fn set_player(&self, player: Option<players::Model>) {
        let session_data = &mut *self.data.write().await;

        let existing = match player {
            Some(player) => {
                let debug_state = format!(
                    "Name: {}, ID: {}, SID: {}",
                    player.display_name, player.id, self.id
                );
                self.set_debug_state(debug_state).await;
                session_data.player.replace(player)
            }
            None => {
                let debug_state = format!("SID: {}", self.id);
                self.set_debug_state(debug_state).await;
                session_data.player.take()
            }
        };

        if let Some(existing) = existing {
            debug!(
                "Swapped authentication from:\nPrevious (ID: {}, Username: {}, Email: {})",
                existing.id, existing.display_name, existing.email,
            );
        }
    }

    /// Attempts to get the session token stored on the database
    /// player object attached to this session but if there is not
    /// one it will create a new session token and update the player
    pub async fn session_token(&self) -> BlazeResult<String> {
        {
            let session_data = self.data.read().await;
            let Some(player) = session_data.player.as_ref() else {
                debug!("Attempted to load session token while not authenticated (SID: {})", self.id);
                return Err(BlazeError::MissingPlayer)
            };
            if let Some(token) = player.session_token.as_ref() {
                return Ok(token.clone());
            }
        }

        let token = generate_random_string(128);
        let session_data = &mut *self.data.write().await;
        let player = session_data
            .player
            .take()
            .ok_or(BlazeError::MissingPlayer)?;
        let (player, token) = PlayersInterface::set_token(self.db(), player, token).await?;
        let _ = session_data.player.insert(player);
        Ok(token)
    }

    /// Sets the game details for the current session
    ///
    /// `game` The game the player has joined.
    /// `slot` The slot in the game the player is in.
    pub async fn set_game(&self, game: u32) {
        let session_data = &mut *self.data.write().await;
        session_data.game = Some(game)
    }

    /// Clears the game details for the current session
    /// returning the game slot if one is present
    pub async fn clear_game(&self) {
        let session_data = &mut *self.data.write().await;
        session_data.game = None;
    }

    /// Updates the data stored on the client so that it matches
    /// the data stored in this session
    pub async fn update_client(&self) {
        let packet = self.create_client_update().await;
        self.write(&packet).await;
    }

    pub async fn create_client_update(&self) -> OpaquePacket {
        let session_data = &*self.data.read().await;
        Packets::notify(
            Components::UserSessions(UserSessions::SetSession),
            &SetSessionDetails {
                session: session_data,
            },
        )
    }

    /// Updates the provided session with the session information
    /// for this session.
    ///
    /// `other` The session to sent the updated details to
    pub async fn update_for(&self, other: &SessionArc) {
        let session_data = &*self.data.read().await;
        let Some(player) = session_data.player.as_ref() else {return;};
        let packets = vec![
            Packets::notify(
                Components::UserSessions(UserSessions::SessionDetails),
                &SessionDetails {
                    session: session_data,
                    player,
                },
            ),
            Packets::notify(
                Components::UserSessions(UserSessions::UpdateExtendedDataAttribute),
                &UpdateExtDataAttr {
                    flags: 0x3,
                    id: player.id,
                },
            ),
        ];
        other.write_all(&packets).await;
    }

    /// Releases the session removing its references from everywhere
    /// that it is stored so that it can be dropped
    pub async fn release(&self) {
        debug!("Releasing Session (SID: {})", self.id);
        self.games().release_player(self).await;
        self.matchmaking().remove(self).await;
        info!("Session was released (SID: {})", self.id);
        self.buffer.flush(self).await;
    }
}

/// Type for session wrapped in Arc
pub type SessionArc = Arc<Session>;

impl Drop for Session {
    fn drop(&mut self) {
        debug!("Session dropped (SID: {})", self.id);
    }
}

/// Structure for buffering packet writes with flushing
/// functionality.
struct SessionBuffer {
    /// Queue of encoded packet bytes behind mutex for thread safety
    queue: Mutex<VecDeque<Vec<u8>>>,
    /// Sender for telling the session processor when the queue needs
    /// to be flushed.
    flush: mpsc::Sender<()>,
}

impl SessionBuffer {
    /// Creates a new session buffer with the provided flush sender
    ///
    /// `flush_send` The sender for sending flush notifications
    pub fn new(flush_send: mpsc::Sender<()>) -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            flush: flush_send,
        }
    }

    /// Writes the contents of the provided packet to the underlying
    /// queue and sends a flush state.
    ///
    /// `packet` The packet to write to the buffer queue.
    async fn write(&self, packet: &OpaquePacket) {
        let queue = &mut *self.queue.lock().await;
        let contents = packet.encode_bytes();
        queue.push_back(contents);
        self.flush.try_send(()).ok();
    }

    /// Writes the contents of the provided packets to the underlying
    /// queue and sends a flush state. Function for writing multiple
    /// without having to aquire the lock again or sending multiple flushes
    ///
    /// `packets` The packets to write to the buffer queue.
    async fn write_all(&self, packets: &Vec<OpaquePacket>) {
        let queue = &mut *self.queue.lock().await;
        for packet in packets {
            let contents = packet.encode_bytes();
            queue.push_back(contents);
        }
        self.flush.try_send(()).ok();
    }

    /// Flushes the contents of the queue writing them to the stream
    /// on the provided `session` if the queue is not empty.
    ///
    /// `session` The session containing the stream to flush the buffer too
    async fn flush(&self, session: &Session) {
        let queue = &mut *self.queue.lock().await;
        if queue.is_empty() {
            return;
        }
        // Counter for the number of items written
        let mut write_count = 0usize;
        let stream = &mut *session.stream.lock().await;

        while let Some(item) = queue.pop_front() {
            match stream.write_all(&item).await {
                Ok(_) => {
                    write_count += 1;
                }
                Err(err) => {
                    error!(
                        "Error occurred while flushing session (SID: {}): {:?}",
                        session.id, err
                    );
                    return;
                }
            }
        }

        debug!(
            "Flushed session (SID: {}, Count: {})",
            session.id, write_count
        )
    }
}

/// Structure for storing session data that is mutated often. This
/// data is placed behind a RwLock so it can be modified.
pub struct SessionData {
    /// If the session is authenticated it will have a linked
    /// player model from the database
    pub player: Option<players::Model>,

    /// Networking information
    pub net: NetData,

    /// Hardware flag name might be incorrect usage is unknown
    pub hardware_flag: u16,

    // Appears to be some sort of client state. Needs further documentation
    pub state: u8,

    /// Matchmaking state if the player is matchmaking.
    pub matchmaking: bool,

    /// The id of the game if connected to one
    pub game: Option<u32>,
}

impl Default for SessionData {
    fn default() -> Self {
        Self {
            player: None,
            net: NetData::default(),
            hardware_flag: 0,
            state: 2,
            matchmaking: false,
            game: None,
        }
    }
}

impl SessionData {
    /// Retrieves the `display_name` of the player attached to this
    /// session data or if there is no player attached an empty string.
    pub fn name_safe(&self) -> String {
        self.player
            .as_ref()
            .map(|value| value.display_name.clone())
            .unwrap_or_else(|| String::new())
    }

    /// Retrieves the `id` of the player attached to this
    /// session data or if there is no player attached the value 1.
    pub fn id_safe(&self) -> u32 {
        self.player.as_ref().map(|value| value.id).unwrap_or(1)
    }
}
