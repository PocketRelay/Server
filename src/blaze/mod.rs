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
use crate::database::entities::Player;
use crate::GlobalState;

mod routes;
pub mod components;

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
    pub player: Option<Player>,
    pub location: u32,
    pub net: Option<NetDetails>,
    pub net_ext: Option<NetExt>,
    pub last_ping: SystemTime,
}

impl SessionImpl {
    fn new(global: Arc<GlobalState>, id: u32, stream: TcpStream, addr: SocketAddr) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            global,
            id,
            stream: RwLock::new(stream),
            addr,
            player: None,
            location: 0x64654445,
            net: None,
            net_ext: None,
            last_ping: SystemTime::now()
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

pub async fn write_packet(session: &Session, packet: OpaquePacket) -> io::Result<()> {
    let mut session = session.read().await;
    let mut stream = session.stream.write().await;
    let stream = stream.deref_mut();
    packet.write_async(stream).await
}

async fn read_packet(session: &Session) -> PacketResult<(Components, OpaquePacket)> {
    let mut session = session.read().await;
    let mut stream = session.stream.write().await;
    let stream = stream.deref_mut();
    OpaquePacket::read_async_typed(stream).await
}

async fn process_session(session: Session) {
    loop {
        let (component, packet)= match read_packet(&session).await {
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
