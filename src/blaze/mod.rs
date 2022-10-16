use std::net::SocketAddr;
use std::ops::DerefMut;
use std::sync::{Arc};
use std::time::SystemTime;
use blaze_pk::{OpaquePacket, PacketContent, PacketResult};
use log::{error, info};
use tokio::io;
use tokio::sync::RwLock;
use tokio::net::{TcpListener, TcpStream};
use crate::blaze::components::Components;
use crate::database::entities::PlayerModel;
use crate::database::interface::DbResult;
use crate::database::interface::players::set_session_token_impl;
use crate::GlobalState;

mod routes;
pub mod components;
pub mod errors;

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
        info!("New Session Started (ID: {}, ADDR: {:?})", session_id, &addr);
        let session = SessionImpl::new(global.clone(), session_id, stream, addr);
        session_id += 1;
        sessions.push(session.clone());
        tokio::spawn(process_session(session));
    }
}

pub type Session = Arc<RwLock<SessionImpl>>;

pub struct SessionImpl {
    pub global: Arc<GlobalState>,
    pub id: u32,
    pub stream: RwLock<TcpStream>,
    pub addr: SocketAddr,
    pub player: Option<PlayerModel>,
    pub location: u32,
    pub net: Option<NetDetails>,
    pub net_ext: Option<NetExt>,
    pub last_ping: SystemTime,
}

impl SessionImpl {
    /// This function creates a new session from the provided values and wraps
    /// the session in the necessary locks and Arc
    fn new(global: Arc<GlobalState>, id: u32, stream: TcpStream, addr: SocketAddr) -> Session {
        Arc::new(RwLock::new(Self {
            global,
            id,
            stream: RwLock::new(stream),
            addr,
            player: None,
            location: 0x64654445,
            net: None,
            net_ext: None,
            last_ping: SystemTime::now(),
        }))
    }
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

/// Updates the session token for the provided session. This involves updating the model
/// in the database by taking it out of the session player and then returning the newly
/// updated player back into the session.
pub async fn set_session_token(session: &Session, token: Option<String>) -> DbResult<()> {
    let mut session = session.write().await;
    if let Some(player) = session.player.take() {
        let _ = session.player.insert(set_session_token_impl(
            &session.global.db,
            player,
            token
        ));
    }
    Ok(())
}

/// Function for asynchronously writing a packet to the provided session. Acquires the
/// required locks and writes the packet to the stream.
pub async fn write_packet(session: &Session, packet: OpaquePacket) -> io::Result<()> {
    let session = session.read().await;
    let mut stream = session.stream.write().await;
    let stream = stream.deref_mut();
    packet.write_async(stream).await
}

/// Function for asynchronously reading a packet from the provided session. Acquires the
/// required locks and reads a packet returning the Component and packet.
async fn read_packet(session: &Session) -> PacketResult<(Components, OpaquePacket)> {
    let session = session.read().await;
    let mut stream = session.stream.write().await;
    let stream = stream.deref_mut();
    OpaquePacket::read_async_typed(stream).await
}

/// Function for processing a session loops until the session is no longer readable.
/// Reads packets and routes them with the routing function.
async fn process_session(session: Session) {
    loop {
        let (component, packet) = match read_packet(&session).await {
            Ok(value) => value,
            Err(_) => break
        };

        match routes::route(&session, component, packet).await {
            Ok(_) => {}
            Err(err) => {
                let session = session.read().await;

                error!("Session {} got err {:?} while routing", session.id, err)
            }
        }
    }
}
