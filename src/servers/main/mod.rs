use self::session::{Session, SessionLink};
use crate::utils::{components::Components, env};
use blaze_pk::{packet::PacketCodec, router::Router};
use interlink::prelude::*;
use log::{error, info};
use tokio::net::TcpListener;
use tokio_util::codec::{FramedRead, FramedWrite};

mod models;
mod routes;
pub mod session;

static mut ROUTER: Option<Router<Components, SessionLink>> = None;

fn router() -> &'static Router<Components, SessionLink> {
    unsafe {
        match &ROUTER {
            Some(value) => value,
            None => panic!("Main server router not yet initialized"),
        }
    }
}

/// Starts the main server which is responsible for a majority of the
/// game logic such as games, sessions, etc.
pub async fn start_server() {
    // Initializing the underlying TCP listener
    let listener = {
        let port = env::from_env(env::MAIN_PORT);
        match TcpListener::bind(("0.0.0.0", port)).await {
            Ok(value) => {
                info!("Started Main server (Port: {})", port);
                value
            }
            Err(_) => {
                error!("Failed to bind Main server (Port: {})", port);
                panic!()
            }
        }
    };

    unsafe {
        ROUTER = Some(routes::router());
    }

    let mut session_id = 1;
    // Accept incoming connections
    loop {
        let (stream, socket_addr) = match listener.accept().await {
            Ok(value) => value,
            Err(err) => {
                error!("Failed to accept Main connection: {err:?}");
                continue;
            }
        };

        Session::create(|ctx| {
            // Attach reader and writers to the session context
            let (read, write) = stream.into_split();
            let read = FramedRead::new(read, PacketCodec);
            let write = FramedWrite::new(write, PacketCodec);

            ctx.attach_stream(read, true);
            let writer = ctx.attach_sink(write);

            Session::new(session_id, socket_addr, writer)
        });

        session_id += 1;
    }
}
