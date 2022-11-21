use log::info;
use tokio::{join, signal, sync::watch};

use core::{env, GlobalState};

use http_server;
use main_server;
use mitm_server;
use redirector_server;

use dotenvy::dotenv;

mod logging;

#[tokio::main]
async fn main() {
    // Load environment variables from nearest .env
    dotenv().ok();

    // Initialize logging
    logging::setup();

    info!("Starting Pocket Relay v{}", env::VERSION);

    // Channel for safely shutdown
    let (shutdown_send, shutdown_recv) = watch::channel(());

    // Initialize global state
    let global_state = &GlobalState::init(shutdown_recv).await;

    // Spawn a handler for safe shutdown
    tokio::spawn(async move {
        signal::ctrl_c().await.ok();
        shutdown_send.send(()).ok();
    });

    let mitm_enabled = env::from_env(env::MITM_ENABLED);
    if mitm_enabled {
        // MITM Mode only requires the Redirector & MITM servers
        join!(
            redirector_server::start_server(global_state),
            mitm_server::start_server(global_state)
        );
    } else {
        // Normal mode requires the Redirector, HTTP, and Main servers
        join!(
            redirector_server::start_server(global_state),
            http_server::start_server(global_state),
            main_server::start_server(global_state)
        );
    }
}
