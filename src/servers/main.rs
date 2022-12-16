use crate::utils::net::{accept_stream, listener};
use crate::{env, state::GlobalState};
use session::Session;
use tokio::sync::mpsc;

mod models;
mod routes;
mod session;

/// Starts the main server which is responsible for a majority of the
/// game logic such as games, sessions, etc.
pub async fn start_server() {
    let listener = listener("Main", env::from_env(env::MAIN_PORT)).await;
    let mut shutdown = GlobalState::shutdown();
    let mut session_id = 1;
    while let Some(values) = accept_stream(&listener, &mut shutdown).await {
        let (message_sender, message_recv) = mpsc::channel(20);
        let session = Session::new(session_id, values, message_sender);
        tokio::spawn(session.process(message_recv));
        session_id += 1;
    }
}
