use core::{env, state::GlobalState};

mod codec;
mod routes;
mod session;

use session::Session;
use tokio::sync::mpsc;
use utils::net::{accept_stream, listener};

/// Starts the Blaze server
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
