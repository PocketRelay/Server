use std::net::SocketAddr;
use std::ops::DerefMut;
use std::sync::{Arc};
use blaze_pk::{OpaquePacket, PacketResult};
use derive_more::From;
use log::error;
use tokio::io;
use tokio::sync::RwLock;
use tokio::net::{TcpListener, TcpStream};
use crate::Components;

mod router;
mod routes;
pub mod components;

#[derive(Debug, From)]
pub enum ServerError {
    IO(io::Error),
}

type ServerResult<T> = Result<T, ServerError>;

pub async fn start_server() -> ServerResult<()> {

    let listener = TcpListener::bind(("0.0.0.0", 14219))
        .await?;

    let mut sessions = Vec::new();

    loop {
        let (stream, addr) = listener.accept().await?;
        let session = Session { stream: RwLock::new(stream), addr };
        let session = Arc::new(session);
        sessions.push(session.clone());
        tokio::spawn(async move {
            let _ = process_client(session).await;
        });
        println!("New socket connection")
    }
}

pub struct Session {
    stream: RwLock<TcpStream>,
    addr: SocketAddr,
}

async fn process_client(session: Arc<Session>) -> PacketResult<()> {
    loop {
        let mut stream = session.stream.write().await;
        let stream = stream.deref_mut();
        let (component, packet) = OpaquePacket::read_async_typed::<Components, _>(stream)
            .await?;
        match router::route(session.clone(), component, packet)
            .await {
            Ok(_) => {}
            Err(err) => {
                error!("{err:?}")
            }
        }
    }
}