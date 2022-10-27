use std::net::SocketAddr;
use std::ops::DerefMut;
use std::sync::{Arc};
use std::time::SystemTime;
use blaze_pk::{Blob, Codec, OpaquePacket, PacketResult, Packets, TdfMap, VarIntList};
use log::{debug, error, info};
use rand::{Rng, thread_rng};
use sea_orm::DatabaseConnection;
use tokio::io;
use tokio::sync::RwLock;
use tokio::net::{TcpListener, TcpStream};
use crate::blaze::components::{Components, UserSessions};
use errors::HandleResult;
use crate::blaze::errors::{BlazeError, BlazeResult};
use crate::blaze::shared::{NetData, SessionDataCodec, SessionDetails, SessionUser, UpdateExtDataAttr};
use crate::database::entities::PlayerModel;
use crate::database::interface::players::set_session_token;
use crate::GlobalState;

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

    let mut sessions = Vec::new();
    let mut session_id = 0;

    loop {
        let (stream, addr) = listener.accept().await?;
        let session = Session::new(global.clone(), session_id, stream, addr);
        let session = Arc::new(session);
        info!("New Session Started (ID: {}, ADDR: {:?})", session.id, session.addr);
        session_id += 1;
        sessions.push(session.clone());
        tokio::spawn(process_session(session));
    }
}

/// Function for processing a session loops until the session is no longer readable.
/// Reads packets and routes them with the routing function.
async fn process_session(session: Arc<Session>) {
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
}

pub struct Session {
    pub global: Arc<GlobalState>,
    pub id: u32,
    pub stream: RwLock<TcpStream>,
    pub addr: SocketAddr,
    pub data: RwLock<SessionData>,
}

#[derive(Debug)]
pub struct SessionData {
    // Basic
    pub player: Option<PlayerModel>,
    pub location: u32,
    pub last_ping: SystemTime,
    // Networking
    pub net: NetData,
    pub hardware_flag: u16,
    pub pslm: u32,
}

impl Session {
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
            }),
        }
    }

    pub async fn update_for(&self, other: &Session) -> BlazeResult<()> {
        let data = self.data.read().await;
        let user = data.user()?;
        let update_ext_data = Packets::notify(
            Components::UserSessions(UserSessions::UpdateExtendedDataAttribute),
            &UpdateExtDataAttr {
                flags: 0x3,
                id: user.id,
            },
        );
        let session_details = Packets::notify(
            Components::UserSessions(UserSessions::SessionDetails),
            &SessionDetails {
                data: data.to_codec(),
                user,
            },
        );

        drop(data);
        other.write_packet(&session_details).await?;
        other.write_packet(&update_ext_data).await?;
        Ok(())
    }

    /// Returns a reference to the database connection from the global
    /// state data.
    pub fn db(&self) -> &DatabaseConnection { &self.global.db }

    /// Generates a session token by getting 128 random alphanumeric
    /// characters and creating a string from them.
    fn generate_token() -> String {
        thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(128)
            .map(char::from)
            .collect()
    }

    /// Obtains the session token for the player linked to this session
    /// optionally setting and returning a new session token if there is
    /// not already one.
    pub async fn session_token(&self) -> BlazeResult<String> {
        let session_data = self.data.read().await;
        debug!("Got read lock for session token");
        let player = session_data.expect_player()?;
        match player.session_token.as_ref() {
            None => {
                drop(session_data);
                let new_token = Self::generate_token();
                self.set_token(Some(new_token.clone()))
                    .await?;
                Ok(new_token)
            }
            Some(token) => Ok(token.clone())
        }
    }

    /// Updates the session token for the provided session. This involves updating the model
    /// in the database by taking it out of the session player and then returning the newly
    /// updated player back into the session.
    pub async fn set_token(&self, token: Option<String>) -> BlazeResult<()> {
        let mut session_data = self.data.write().await;
        match session_data.player.take() {
            Some(player) => {
                let player = set_session_token(self.db(), player, token).await?;
                let _ = session_data.player.insert(player);
                Ok(())
            }
            None => return Err(BlazeError::MissingPlayer)
        }
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
}

impl SessionData {

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

    pub fn user(&self) -> BlazeResult<SessionUser> {
        let player = self.player
            .as_ref()
            .ok_or(BlazeError::MissingPlayer)?;
        Ok(SessionUser {
            aid: player.id,
            location: self.location,
            exbb: Blob::empty(),
            exid: 0,
            id: player.id,
            name: player.display_name.clone(),
        })
    }

    pub fn to_codec(&self) -> SessionDataCodec {
        SessionDataCodec {
            addr: self.net.get_groups(),
            bps: "ea-sjc",
            cty: "",
            cvar: VarIntList::empty(),
            dmap: TdfMap::only(0x70001u32, 0x409au32),
            hardware_flag: self.hardware_flag,
            pslm: vec![self.pslm],
            net_ext: self.net.ext,
            uatt: 0,
            ulst: vec![(0, 0, self.game_id())],
        }
    }

    /// Function for retrieving the ID of the current game that this player
    /// is apart of (currently always zero)
    pub fn game_id(&self) -> u32 {
        0
    }
}