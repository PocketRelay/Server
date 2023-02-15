use self::session::{Session, SessionReader, SessionWriter};
use crate::utils::{
    actor::{Actor, Addr},
    components::Components,
    env,
};
use blaze_pk::router::Router;
use log::{error, info};
use tokio::net::TcpListener;

mod models;
mod routes;
pub mod session;

static mut ROUTER: Option<Router<Components, Addr<Session>>> = None;

fn router() -> &'static Router<Components, Addr<Session>> {
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

        Session::create(
            |ctx| {
                // Attach reader and writers to the session context
                let (read, write) = stream.into_split();
                let writer = SessionWriter::new(write, ctx.addr());
                SessionReader::new(read, ctx.addr());

                Session::new(session_id, socket_addr, writer)
            },
            session_id,
        );

        session_id += 1;
    }
}
