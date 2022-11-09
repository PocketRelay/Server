use crate::blaze::components::{Components, GameManager, UserSessions};
use crate::blaze::errors::{BlazeError, BlazeResult};
use crate::blaze::shared::{
    NetData, SessionDetails, SessionStateChange, SetSessionDetails, UpdateExtDataAttr,
};
use crate::database::entities::PlayerModel;
use crate::database::interface::players::set_session_token;
use crate::env;
use crate::game::matchmaking::Matchmaking;
use crate::game::{Game, GameArc, Games};
use crate::retriever::Retriever;
use crate::utils::generate_token;
use crate::GlobalState;
use blaze_pk::{Codec, OpaquePacket, PacketComponents, PacketResult, PacketType, Packets, Tag, Reader};
use errors::HandleResult;
use log::{debug, error, info};
use sea_orm::DatabaseConnection;
use tokio::io::AsyncWriteExt;
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, Mutex, mpsc};
use tokio::{io, select};

pub mod components;
pub mod errors;
mod routes;
pub mod shared;

/// Starts the main Blaze server with the provided global state.
pub async fn start_server(global: Arc<GlobalState>) -> io::Result<()> {
    let main_port = env::u16_env(env::MAIN_PORT);
    info!("Starting Main Server on (0.0.0.0:{main_port})");
    let listener = TcpListener::bind(("0.0.0.0", main_port)).await?;

    let mut session_id = 1;
    let mut shutdown_recv = global.shutdown_recv.resubscribe();

    loop {
        let (stream, addr) = select! {
            value = listener.accept() => value?,
            _ = shutdown_recv.recv() => break,
        };
        let (flush_send, flush_recv) = mpsc::channel(1);
        let session = Session::new(global.clone(), session_id, stream, addr, flush_send);
        let session = Arc::new(session);
        info!(
            "New Session Started (ID: {}, ADDR: {:?})",
            session.id, session.addr
        );
        session_id += 1;
        tokio::spawn(process_session(session, flush_recv));
    }
    Ok(())
}

/// Function for processing a session loops until the session is no longer readable.
/// Reads packets and routes them with the routing function.
async fn process_session(session: SessionArc, mut flush_recv: mpsc::Receiver<bool>) {
    let mut shutdown_recv = session.global.shutdown_recv.resubscribe();
    loop {
        let (component, packet) = select! {
            _ = flush_recv.recv() => {
                session.flush().await;
                continue; 
            }
            res = session.read_packet() => {
                    match res {
                    Ok(value) => value,
                    Err(_) => break,
                }
            }
            _ = shutdown_recv.recv() => {break;},
        };

        session.debug_log_packet("Read", &packet).await;

        match routes::route(&session, component, &packet).await {
            Ok(_) => {}
            Err(err) => {
                error!("Session {} got err {:?} while routing", session.id, err)
            }
        }

        session.flush().await;
    }
    session.release().await;
}



pub struct Session {
    pub global: Arc<GlobalState>,
    pub id: u32,
    pub stream: Mutex<TcpStream>,
    pub addr: SocketAddr,
    pub data: RwLock<SessionData>,

    pub flush_sender: mpsc::Sender<bool>,
    pub write_buffer: Mutex<VecDeque<Vec<u8>>>,
    
    // Debug logging extra information
    pub debug_state: RwLock<String>,
}

impl Drop for Session {
    fn drop(&mut self) {
        debug!("Session with id {} dropped", self.id)
    }
}

// Type for session wrapped in an arc
pub type SessionArc = Arc<Session>;

pub struct SessionData {
    // Basic
    pub player: Option<PlayerModel>,
    pub location: u32,
    pub last_ping: SystemTime,

    // Networking
    pub net: NetData,
    pub hardware_flag: u16,
    pub pslm: u32,

    pub state: u8,
    pub matchmaking: Option<MatchmakingState>,

    // Game Details
    pub game: Option<SessionGame>,
}

impl Default for SessionData {
    fn default() -> Self {
        Self {
            player: None,
            location: 0x64654445,
            last_ping: SystemTime::now(),
            net: NetData::default(),
            hardware_flag: 0,
            pslm: 0xfff0fff,
            state: 2,
            matchmaking: None,
            game: None,
        }
    }
}

pub struct MatchmakingState {
    pub id: u32,
    pub start: u64,
}

impl Default for MatchmakingState {
    fn default() -> Self {
        Self { id: 1, start: 0 }
    }
}

pub struct SessionGame {
    /// Reference to the connected game
    pub game: Arc<Game>,
    /// Slot index of this session in the game
    pub slot: usize,
}

impl Session {
    /// This function creates a new session from the provided values and wraps
    /// the session in the necessary locks and Arc
    fn new(global: Arc<GlobalState>, id: u32, stream: TcpStream, addr: SocketAddr, flush_sender: mpsc::Sender<bool>) -> Session {
        Self {
            global,
            id,
            stream: Mutex::new(stream),
            addr,
            flush_sender,
            data: RwLock::new(SessionData::default()),
            write_buffer: Mutex::new(VecDeque::new()),
            debug_state: RwLock::new(format!("ID: {}", id))
        }
    }

    pub async fn set_state(&self, state: u8) {
        let mut data = self.data.write().await;
        data.state = state;

        let Some(player) = &data.player else {return;};
        let Some(sess_game) = &data.game else {return;};

        let game = &sess_game.game;
        let packet = Packets::notify(
            Components::GameManager(GameManager::GamePlayerStateChange),
            &SessionStateChange {
                gid: game.id,
                pid: player.id,
                state,
            },
        );
        game.push_all(&packet).await;
    }

    pub async fn release(&self) {
        debug!("Releasing session {}", self.id);
        self.games().release_player(self).await;
        info!("Session {} was released", self.id);
        self.flush().await;
    }

    pub async fn update_for(&self, other: &SessionArc) {
        {
            let data = self.data.read().await;
            let Some(player) = &data.player else { return; };
            other.notify(
                Components::UserSessions(UserSessions::SessionDetails) , 
                &SessionDetails {
                    session: &data,
                    player,
                }
            ).await;

            other.notify(
            Components::UserSessions(UserSessions::UpdateExtendedDataAttribute),
                &UpdateExtDataAttr {
                    flags: 0x3,
                    id: player.id,
                },
            ).await;
        }   
    }

    /// Sends a Components::UserSessions(UserSessions::SetSession) packet to the client updating
    /// the clients session information with the copy stored on the server.
    pub async fn update_client(&self) {
        let session_data = self.data.read().await;
        self.notify(
            Components::UserSessions(UserSessions::SetSession),
            &SetSessionDetails {
                session: &session_data,
            },
        ).await;
    }

    /// Returns a reference to the database connection from the global
    /// state data.
    pub fn db(&self) -> &DatabaseConnection {
        &self.global.db
    }

    /// Returns a reference to the retriever instance if there is one
    /// present.
    pub fn retriever(&self) -> Option<&Retriever> {
        self.global.retriever.as_ref()
    }

    /// Returns a reference to the games manager from the global
    /// state data.
    pub fn games(&self) -> &Games {
        &self.global.games
    }

    /// Returns a reference to the matchmaking manager from the global
    /// state data.
    pub fn matchmaking(&self) -> &Matchmaking {
        &self.global.matchmaking
    }

    /// Obtains the session token for the player linked to this session
    /// optionally setting and returning a new session token if there is
    /// not already one.
    pub async fn session_token(&self) -> BlazeResult<String> {
        {
            let session_data = self.data.read().await;
            let player = session_data.expect_player()?;
            if let Some(token) = &player.session_token {
                return Ok(token.clone());
            }
        }

        let token = generate_token(128);
        let mut session_data = self.data.write().await;
        let player = session_data
            .player
            .take()
            .ok_or(BlazeError::MissingPlayer)?;
        let (player, token) = set_session_token(self.db(), player, token).await?;
        let _ = session_data.player.insert(player);
        Ok(token)
    }

    /// Sets the current game value for this session
    pub async fn set_game(&self, game: GameArc, slot: usize) {
        let mut session_data = self.data.write().await;
        session_data.game = Some(SessionGame { game, slot });
    }

    pub async fn clear_game(&self) {
        let mut session_data = self.data.write().await;
        session_data.game = None;
    }

    async fn set_debug_state(&self, value: &str) {
        let state = &mut *self.debug_state.write().await;
        state.clear();
        state.push_str(value);
    
    }

    /// Sets the player stored in this session to the provided player. This
    /// wrapper allows state that depends on this session having a player to
    /// be updated accordingly such as games
    pub async fn set_player(&self, player: Option<PlayerModel>) {
        let mut session_data = self.data.write().await;
        let existing = if let Some(player) = player {
            self.set_debug_state(&format!("Name: {}, ID: {}", player.display_name, player.id)).await;
            session_data.player.replace(player)
        } else {
            self.set_debug_state(&format!("ID: {}", self.id)).await;
            session_data.player.take()
        };
        if let Some(existing) = existing {
            debug!("Swapped authentication from: ");
            debug!("ID = {}", &existing.id);
            debug!("Username = {}", &existing.display_name);
            debug!("Email = {}", &existing.email);
        }
    }

    pub async fn debug_log_packet(&self, action: &str, packet: &OpaquePacket) {
        if !log::log_enabled!(log::Level::Debug) {
            return;     
        }
        let header = &packet.0;
        let component = Components::from_values(
            header.component,
            header.command,
            header.ty == PacketType::Notify,
        );

        let debug_info = &*self.debug_state.read().await;

        // Filter out ping packets entirely as they happen often and don't contain
        // any relevant info.
        if component == Components::Util(components::Util::Ping) 
        || component == Components::Util(components::Util::SuspendUserPing) {
            return;
        }

        // Filter out packets we don't want to log because they are often large
        if component == Components::Authentication(components::Authentication::ListUserEntitlements2)
        || component == Components::Util(components::Util::FetchClientConfig)
        || component == Components::Util(components::Util::UserSettingsLoadAll) {
            // Write bare minimum information
            debug!("\nSession {} Packet\nInfo: ({})\nComponent: {:?}\nType: {:?}", action, debug_info, component, header.ty);
            return;
        }


        let mut reader = Reader::new(&packet.1);
        let mut out = String::new();
        out.push_str("{\n");
        match Tag::stringify(&mut reader, &mut out, 1) {
            Ok(_) => {},
            Err(err) => {
                // Include decoding error in message
                debug!(
                    "\nSession {} Packet\nInfo: ({})\nComponent: {:?}\nType: {:?}\nExtra: Content was malformed\nError: {:?}\nPartial Content: {}",
                    action, 
                    debug_info,
                    component, 
                    header.ty, 
                    err,
                    out
                );
                return;
            }
        };
        if out.len() == 2 {
            // Remove new line if nothing else was appended
            out.pop();
        }
        out.push('}');
        debug!("\nSession {} Packet\nInfo: ({})\nComponent: {:?}\nType: {:?}\nContent: {}", action, debug_info, component, header.ty, out);
    }

    /// Function for asynchronously writing a packet to the provided session. Acquires the
    /// required locks and writes the packet to the stream.
    pub async fn write_packet(&self, packet: &OpaquePacket) {
        self.debug_log_packet("Queued Write", packet).await;
        let write_queue = &mut *self.write_buffer.lock().await;
        let contents = packet.encode_bytes();
        write_queue.push_back(contents);
        self.flush_sender.try_send(true).ok();
    }

    pub async fn write_packet_direct(&self, packet: &OpaquePacket) -> io::Result<()> {
        let stream = &mut *self.stream.lock().await;
        packet.write_async(stream).await?;
        self.debug_log_packet("Wrote", packet).await;
        Ok(())
    }

    /// Writes all the provided packets in order.
    pub async fn write_packets(&self, packets: &Vec<OpaquePacket>) {
        let write_queue = &mut *self.write_buffer.lock().await;
        for packet in packets {
            self.debug_log_packet("Queued Write", packet).await;
            let contents = packet.encode_bytes();
            write_queue.push_back(contents);
        }
        self.flush_sender.try_send(true).ok();
    }

    pub async fn flush(&self) {
        let write_queue = &mut *self.write_buffer.lock().await;
        if write_queue.is_empty() {
            return;
        }
        let stream = &mut *self.stream.lock().await;
        while let Some(item) = write_queue.pop_front() {
            match stream.write_all(&item).await {
                Ok(_) => {},
                Err(err) => {
                    error!("Error while flushing session (ID: {}): {:?}", self.id, err);
                    return;
                }
            }
        }  
    }


    pub async fn notify<T: Codec>(&self, component: Components, contents: &T) {
        self.write_packet(&Packets::notify(component, contents))
            .await;
    }

    pub async fn notify_immediate<T: Codec>(&self, component: Components, contents: &T) -> HandleResult {
        self.write_packet_direct(&Packets::notify(component, contents))
            .await?;
        Ok(())
    }

    /// Function for asynchronously reading a packet from the provided session. Acquires the
    /// required locks and reads a packet returning the Component and packet.
    async fn read_packet(&self) -> PacketResult<(Components, OpaquePacket)> {
        let stream = &mut *self.stream.lock().await;
        OpaquePacket::read_async_typed(stream).await
    }

    #[inline]
    pub async fn response<T: Codec>(&self, packet: &OpaquePacket, contents: &T) -> HandleResult {
        self.write_packet_direct(&Packets::response(packet, contents))
            .await?;
        Ok(())
    }

    #[inline]
    pub async fn response_empty(&self, packet: &OpaquePacket) -> HandleResult {
        self.write_packet_direct(&Packets::response_empty(packet)).await?;
        Ok(())
    }

    #[inline]
    pub async fn response_error<T: Codec>(
        &self,
        packet: &OpaquePacket,
        error: impl Into<u16>,
        contents: &T,
    ) -> HandleResult {
        self.write_packet_direct(&Packets::error(packet, error, contents))
            .await?;
        Ok(())
    }

    #[inline]
    pub async fn response_error_empty(
        &self,
        packet: &OpaquePacket,
        error: impl Into<u16>,
    ) -> HandleResult {
        self.write_packet_direct(&Packets::error_empty(packet, error))
            .await?;
        Ok(())
    }

    pub async fn player_id(&self) -> Option<u32> {
        let session_data = self.data.read().await;
        session_data.player.as_ref().map(|player| player.id)
    }

    pub async fn player_id_safe(&self) -> u32 {
        let session_data = self.data.read().await;
        session_data.player_id_safe()
    }

    pub async fn expect_player_id(&self) -> BlazeResult<u32> {
        let session_data = self.data.read().await;
        let player = session_data.expect_player()?;
        Ok(player.id)
    }
}

impl SessionData {
    pub fn player_name_safe(&self) -> String {
        self.player
            .as_ref()
            .map(|value| value.display_name.clone())
            .unwrap_or_else(|| String::new())
    }

    pub fn player_id_safe(&self) -> u32 {
        self.player.as_ref().map(|value| value.id).unwrap_or(1)
    }

    pub fn game_id_safe(&self) -> u32 {
        self.game.as_ref().map(|game| game.game.id).unwrap_or(1)
    }

    pub fn game_slot_safe(&self) -> usize {
        self.game.as_ref().map(|game| game.slot).unwrap_or(1)
    }

    pub fn expect_player(&self) -> BlazeResult<&PlayerModel> {
        self.player.as_ref().ok_or(BlazeError::MissingPlayer)
    }

    pub fn expect_player_owned(&mut self) -> BlazeResult<PlayerModel> {
        self.player.take().ok_or(BlazeError::MissingPlayer)
    }

    /// Function for retrieving the ID of the current game that this player
    /// is apart of (currently always zero)
    pub fn game_id(&self) -> u32 {
        0
    }
}
