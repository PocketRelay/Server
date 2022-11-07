use crate::blaze::components::{Components, GameManager, UserSessions};
use crate::blaze::errors::{BlazeError, BlazeResult};
use crate::blaze::shared::{
    NetData, SessionDetails, SessionStateChange, SetSessionDetails, UpdateExtDataAttr,
};
use crate::database::entities::PlayerModel;
use crate::database::interface::players::set_session_token;
use crate::env;
use crate::game::matchmaking::Matchmaking;
use crate::game::{Game, Games};
use crate::retriever::Retriever;
use crate::utils::generate_token;
use crate::GlobalState;
use blaze_pk::{Codec, OpaquePacket, PacketResult, Packets};
use errors::HandleResult;
use log::{debug, error, info, LevelFilter};
use sea_orm::DatabaseConnection;
use std::net::SocketAddr;
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
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

    let mut session_id = 0;
    let mut shutdown_recv = global.shutdown_recv.resubscribe();

    loop {
        let (stream, addr) = select! {
            value = listener.accept() => value?,
            _ = shutdown_recv.recv() => break,
        };
        let session = Session::new(global.clone(), session_id, stream, addr);
        let session = Arc::new(session);
        info!(
            "New Session Started (ID: {}, ADDR: {:?})",
            session.id, session.addr
        );
        session_id += 1;
        tokio::spawn(process_session(session));
    }
    Ok(())
}

/// Function for processing a session loops until the session is no longer readable.
/// Reads packets and routes them with the routing function.
async fn process_session(session: SessionArc) {
    let mut shutdown_recv = session.global.shutdown_recv.resubscribe();
    loop {
        let (component, packet) = select! {
            res = session.read_packet() => {
                    match res {
                    Ok(value) => value,
                    Err(_) => break,
                }
            }
            _ = shutdown_recv.recv() => {break;},
        };

        match routes::route(&session, component, &packet).await {
            Ok(_) => {}
            Err(err) => {
                error!("Session {} got err {:?} while routing", session.id, err)
            }
        }
    }
    session.release().await;
}

pub struct Session {
    pub global: Arc<GlobalState>,
    pub id: u32,
    pub stream: RwLock<TcpStream>,
    pub addr: SocketAddr,
    pub data: RwLock<SessionData>,
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
    pub async fn set_state(&self, state: u8) -> BlazeResult<()> {
        let mut data = self.data.write().await;
        data.state = state;
        if let Some(sess_game) = &data.game {
            let game = &sess_game.game;
            let packet = Packets::notify(
                Components::GameManager(GameManager::GamePlayerStateChange),
                &SessionStateChange {
                    gid: game.id,
                    pid: self.id,
                    state,
                },
            );
            game.push_all(&packet).await?;
        }
        Ok(())
    }

    pub async fn release(&self) {
        debug!("Releasing session {}", self.id);
        self.games().release_player(self).await;
        info!("Session {} was released", self.id);
    }

    /// This function creates a new session from the provided values and wraps
    /// the session in the necessary locks and Arc
    fn new(global: Arc<GlobalState>, id: u32, stream: TcpStream, addr: SocketAddr) -> Session {
        Self {
            global,
            id,
            stream: RwLock::new(stream),
            addr,
            data: RwLock::new(SessionData::default()),
        }
    }

    pub async fn update_for(&self, other: &SessionArc) -> io::Result<()> {
        let data = self.data.read().await;
        let Some(player) = &data.player else { return Ok(()) };
        let update_ext_data = Packets::notify(
            Components::UserSessions(UserSessions::UpdateExtendedDataAttribute),
            &UpdateExtDataAttr {
                flags: 0x3,
                id: player.id,
            },
        );
        let session_details = Packets::notify(
            Components::UserSessions(UserSessions::SessionDetails),
            &SessionDetails {
                session: &data,
                player,
            },
        );
        other.write_packet(&session_details).await?;
        other.write_packet(&update_ext_data).await?;
        Ok(())
    }

    /// Sends a Components::UserSessions(UserSessions::SetSession) packet to the client updating
    /// the clients session information with the copy stored on the server.
    pub async fn update_client(&self) -> BlazeResult<()> {
        let data = self.data.read().await;
        let res = SetSessionDetails { session: &data };
        let packet = Packets::notify(Components::UserSessions(UserSessions::SetSession), &res);
        self.write_packet(&packet).await?;
        Ok(())
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

    /// Sets the player stored in this session to the provided player. This
    /// wrapper allows state that depends on this session having a player to
    /// be updated accordingly such as games
    pub async fn set_player(&self, player: Option<PlayerModel>) {
        let mut session_data = self.data.write().await;
        let existing = if let Some(player) = player {
            session_data.player.replace(player)
        } else {
            session_data.player.take()
        };
        if let Some(existing) = existing {
            debug!("Swapped authentication from: ");
            debug!("ID = {}", &existing.id);
            debug!("Username = {}", &existing.display_name);
            debug!("Email = {}", &existing.email);
        }
    }

    /// Function for asynchronously writing a packet to the provided session. Acquires the
    /// required locks and writes the packet to the stream.
    pub async fn write_packet(&self, packet: &OpaquePacket) -> io::Result<()> {
        if log::max_level() >= LevelFilter::Debug {
            debug!("Sent packet TY {:?}", &packet.0.ty);
            let _ = packet.debug_decode();
        }
        let mut stream = self.stream.write().await;
        let stream = stream.deref_mut();
        packet.write_async(stream).await
    }

    /// Function for asynchronously reading a packet from the provided session. Acquires the
    /// required locks and reads a packet returning the Component and packet.
    async fn read_packet(&self) -> PacketResult<(Components, OpaquePacket)> {
        let mut stream = self.stream.write().await;
        let stream = stream.deref_mut();
        OpaquePacket::read_async_typed(stream).await
    }

    #[inline]
    pub async fn response<T: Codec>(&self, packet: &OpaquePacket, contents: &T) -> HandleResult {
        self.write_packet(&Packets::response(packet, contents))
            .await?;
        Ok(())
    }

    #[inline]
    pub async fn response_empty(&self, packet: &OpaquePacket) -> HandleResult {
        self.write_packet(&Packets::response_empty(packet)).await?;
        Ok(())
    }

    #[inline]
    pub async fn response_error<T: Codec>(
        &self,
        packet: &OpaquePacket,
        error: impl Into<u16>,
        contents: &T,
    ) -> HandleResult {
        self.write_packet(&Packets::error(packet, error, contents))
            .await?;
        Ok(())
    }

    #[inline]
    pub async fn response_error_empty(
        &self,
        packet: &OpaquePacket,
        error: impl Into<u16>,
    ) -> HandleResult {
        self.write_packet(&Packets::error_empty(packet, error))
            .await?;
        Ok(())
    }

    pub async fn player_id(&self) -> Option<u32> {
        let session_data = self.data.read().await;
        session_data.player.as_ref().map(|player| player.id)
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
