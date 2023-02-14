use self::session::SessionAddr;
use crate::utils::{components::Components, env};
use blaze_pk::router::Router;
use log::{error, info};
use tokio::net::TcpListener;

mod models;
mod routes;
pub mod session;

static mut ROUTER: Option<Router<Components, SessionAddr>> = None;

fn router() -> &'static Router<Components, SessionAddr> {
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
        let values = match listener.accept().await {
            Ok(value) => value,
            Err(err) => {
                error!("Failed to accept Main connection: {err:?}");
                continue;
            }
        };
        SessionAddr::spawn(session_id, values);
        session_id += 1;
    }
}
