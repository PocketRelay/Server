use core::{env, state::GlobalState};

mod codec;
mod routes;
mod session;

use session::Session;
use utils::net::{accept_stream, listener};

/// Starts the Blaze server using the provided global state
/// which is cloned for the spawned sessions.
pub async fn start_server() {
    let listener = listener("Main", env::from_env(env::MAIN_PORT)).await;
    let mut shutdown = GlobalState::shutdown();
    let mut session_id = 1;
    while let Some(values) = accept_stream(&listener, &mut shutdown).await {
        Session::spawn(session_id, values);
        session_id += 1;
    }
}
