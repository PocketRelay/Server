use std::net::SocketAddr;
use std::ops::DerefMut;
use std::sync::{Arc};
use std::time::SystemTime;
use blaze_pk::{OpaquePacket, PacketResult};
use log::{error, info};
use sea_orm::DatabaseConnection;
use tokio::io;
use tokio::sync::RwLock;
use tokio::net::{TcpListener, TcpStream};
use crate::blaze::components::Components;
use crate::database::entities::PlayerModel;
use crate::database::interface::DbResult;
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
    pub data: RwLock<SessionData>
}

pub struct SessionData {
    pub player: Option<PlayerModel>,
    pub location: u32,
    pub last_ping: SystemTime,
    pub net: Option<NetDetails>,
    pub net_ext: Option<NetExt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetDetails {
    internal: NetGroup,
    external: NetGroup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetGroup {
    address: u32,
    port: u16,
}

pub struct NetExt {
    dbps: u16,
    natt_type: u8,
    ubps: u16,
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
                net: None,
                net_ext: None,
                last_ping: SystemTime::now(),
            })
        }
    }

    /// Returns a reference to the database connection from the global
    /// state data.
    pub fn db(&self) -> &DatabaseConnection { &self.global.db }

    /// Updates the session token for the provided session. This involves updating the model
    /// in the database by taking it out of the session player and then returning the newly
    /// updated player back into the session.
    pub async fn set_token(&self, token: Option<String>) -> DbResult<()> {
        let mut session_data = self.data.write().await;
        if let Some(player) = session_data.player.take() {
            let player = set_session_token(self.db(), player, token).await?;
            let _ = session_data.player.insert(player);
        }
        Ok(())
    }

    /// Function for asynchronously writing a packet to the provided session. Acquires the
    /// required locks and writes the packet to the stream.
    pub async fn write_packet(&self, packet: OpaquePacket) -> io::Result<()> {
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
}

