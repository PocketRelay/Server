use std::net::SocketAddr;
use std::ops::DerefMut;
use std::sync::{Arc};
use blaze_pk::{OpaquePacket, PacketResult};
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
        let session = Session::new(global.clone(), session_id, stream, addr);
        info!("New Session Started (ID: {}, ADDR: {:?})", session.id, session.addr);
        session_id += 1;
        let session = Arc::new(session);
        sessions.push(session.clone());
        tokio::spawn(async move {
            let _ = Session::process(session).await;
        });
    }
}

pub struct Session {
    pub global: Arc<GlobalState>,
    pub id: u32,
    pub stream: RwLock<TcpStream>,
    pub addr: SocketAddr,
    pub player: Option<Player>,
    pub net: Option<NetDetails>,
    pub net_ext: Option<NetExt>,
}

impl Session {
    fn new(global: Arc<GlobalState>, id: u32, stream: TcpStream, addr: SocketAddr) -> Self {
        Self {
            global,
            id,
            stream: RwLock::new(stream),
            addr,
            player: None,
            net: None,
            net_ext: None,
        }
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

impl Session {
    async fn process(session: Arc<Session>) -> PacketResult<()> {
        loop {
            // Scoped so we release the lock after reading the packet
            let (component, packet) = {
                let mut stream = session.stream.write().await;
                let stream = stream.deref_mut();
                OpaquePacket::read_async_typed::<Components, _>(stream).await?
            };

            match routes::route(session.clone(), component, packet).await {
                Ok(_) => {}
                Err(err) => {
                    error!("Session {} got err {:?} while routing", session.id, err)
                }
            }
        }
    }
}