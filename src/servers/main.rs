use crate::{
    env,
    utils::net::{accept_stream, listener},
};
use session::Session;

mod models;
mod routes;
pub mod session;

/// Starts the main server which is responsible for a majority of the
/// game logic such as games, sessions, etc.
pub async fn start_server() {
    let listener = listener("Main", env::from_env(env::MAIN_PORT)).await;
    let mut session_id = 1;
    while let Some(values) = accept_stream(&listener).await {
        Session::spawn(session_id, values);
        session_id += 1;
    }
}
