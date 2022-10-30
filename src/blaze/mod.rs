use std::net::SocketAddr;
use std::ops::DerefMut;
use std::sync::{Arc};
use std::time::SystemTime;
use blaze_pk::{Codec, OpaquePacket, PacketResult, Packets};
use log::{debug, error, info};
use sea_orm::DatabaseConnection;
use tokio::io;
use tokio::sync::{Mutex, RwLock};
use tokio::net::{TcpListener, TcpStream};
use crate::blaze::components::{Components, UserSessions};
use errors::HandleResult;
use crate::blaze::errors::{BlazeError, BlazeResult};
use crate::blaze::shared::{NetData, SessionDetails, SetSessionDetails, UpdateExtDataAttr};
use crate::database::entities::PlayerModel;
use crate::database::interface::players::set_session_token;
use crate::game::Game;
use crate::GlobalState;
use crate::utils::generate_token;

mod routes;
pub mod components;
pub mod errors;
pub mod shared;

/// Starts the main Blaze server with the provided global state.
pub async fn start_server(global: Arc<GlobalState>) -> io::Result<()> {
    let main_port = crate::env::main_port();
    info!("Starting Main Server on (0.0.0.0:{main_port})");
    let listener = TcpListener::bind(("0.0.0.0", main_port))
        .await?;

    let mut session_id = 0;

    loop {
        let (stream, addr) = listener.accept().await?;
        let session = Session::new(global.clone(), session_id, stream, addr);
        let session = Arc::new(session);
        info!("New Session Started (ID: {}, ADDR: {:?})", session.id, session.addr);
        session_id += 1;
        tokio::spawn(process_session(session));
    }
}

/// Function for processing a session loops until the session is no longer readable.
/// Reads packets and routes them with the routing function.
async fn process_session(session: SessionArc) {
    loop {
        let (component, packet) = match session.read_packet().await {
            Ok(value) => value,
            Err(_) => break
        };

        match routes::route(&session, component, &packet).await {
            Ok(_) => {}
            Err(err) => {
                error!("Session {} got err {:?} while routing", session.id, err)
            }
        }
    }
    session.release();
}

pub struct Session {
    pub global: Arc<GlobalState>,
    pub id: u32,
    pub stream: RwLock<TcpStream>,
    pub addr: SocketAddr,
    pub data: RwLock<SessionData>,
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

pub struct MatchmakingState {
    pub id: u32,
    pub start: u64,
}

pub struct SessionGame {
    /// Reference to the connected game
    pub game: Arc<Game>,
    /// Slot index of this session in the game
    pub slot: usize,
}

impl Session {
    pub fn release(&self) {
        info!("Session {} was released", self.id)
        // TODO: Release the session removing all references to it
    }

    /// This function creates a new session from the provided values and wraps
    /// the session in the necessary locks and Arc
    fn new(global: Arc<GlobalState>, id: u32, stream: TcpStream, addr: SocketAddr) -> Session {
        Self {
            global,
            id,
            stream: RwLock::new(stream),
            addr,
            data: RwLock::new(SessionData {
                player: None,
                location: 0x64654445,
                last_ping: SystemTime::now(),
                net: NetData::default(),
                hardware_flag: 0,
                pslm: 0xfff0fff,
                state: 2,
                matchmaking: None,
                game: None,
            }),
        }
    }

    pub async fn update_for(&self, other: &SessionArc) -> BlazeResult<()> {
        let data = self.data.read().await;
        let player = data.expect_player()?;
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
        let packet = Packets::notify(
            Components::UserSessions(UserSessions::SetSession),
            &res,
        );
        self.write_packet(&packet).await?;
        Ok(())
    }

    /// Returns a reference to the database connection from the global
    /// state data.
    pub fn db(&self) -> &DatabaseConnection { &self.global.db }

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
        let player = session_data.player.take()
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
        self.write_packet(&Packets::response(packet, contents)).await?;
        Ok(())
    }

    #[inline]
    pub async fn response_empty(&self, packet: &OpaquePacket) -> HandleResult {
        self.write_packet(&Packets::response_empty(packet)).await?;
        Ok(())
    }

    #[inline]
    pub async fn response_error<T: Codec>(&self, packet: &OpaquePacket, error: impl Into<u16>, contents: &T) -> HandleResult {
        self.write_packet(&Packets::error(packet, error, contents)).await?;
        Ok(())
    }

    #[inline]
    pub async fn response_error_empty(&self, packet: &OpaquePacket, error: impl Into<u16>) -> HandleResult {
        self.write_packet(&Packets::error_empty(packet, error)).await?;
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
        self
            .player
            .as_ref()
            .map(|value| value.display_name.clone())
            .unwrap_or_else(|| String::new())
    }

    pub fn player_id_safe(&self) -> u32 {
        self
            .player
            .as_ref()
            .map(|value| value.id)
            .unwrap_or(1)
    }

    pub fn game_id_safe(&self) -> u32 {
        self
            .game
            .as_ref()
            .map(|game| game.game.id)
            .unwrap_or(1)
    }

    pub fn game_slot_safe(&self) -> usize {
        self
            .game
            .as_ref()
            .map(|game| game.slot)
            .unwrap_or(1)
    }

    pub fn expect_player(&self) -> BlazeResult<&PlayerModel> {
        self.player
            .as_ref()
            .ok_or(BlazeError::MissingPlayer)
    }

    pub fn expect_player_owned(&mut self) -> BlazeResult<PlayerModel> {
        self.player
            .take()
            .ok_or(BlazeError::MissingPlayer)
    }

    /// Function for retrieving the ID of the current game that this player
    /// is apart of (currently always zero)
    pub fn game_id(&self) -> u32 {
        0
    }
}