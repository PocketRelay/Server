use core::{env, GlobalStateArc};

mod codec;
mod routes;
mod session;

use session::Session;
use utils::net::{accept_stream, listener};

/// Starts the Blaze server using the provided global state
/// which is cloned for the spawned sessions.
///
/// `global` The global state
pub async fn start_server(global: &GlobalStateArc) {
    let listener = listener("Main", env::from_env(env::MAIN_PORT)).await;
    let mut shutdown = global.shutdown.clone();
    let mut session_id = 1;
    while let Some(values) = accept_stream(&listener, &mut shutdown).await {
        Session::spawn(global.clone(), session_id, values);
        session_id += 1;
    }
}
