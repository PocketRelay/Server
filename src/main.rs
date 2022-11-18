use log::info;
use tokio::{select, signal, sync::watch};

use core::{env, GlobalState, GlobalStateArc};

use http_server;
use main_server;
use mitm_server;
use redirector_server;

use dotenvy::dotenv;

#[tokio::main]
async fn main() {
    dotenv().ok();

    {
        let logging_level = utils::logging::logging_level();
        let logging_path = env::str_env(env::LOGGING_DIR);
        let compress = env::bool_env(env::LOG_COMPRESSION);
        utils::logging::init_logger(logging_level, logging_path, compress);
    }

    let (shutdown_send, shutdown_recv) = watch::channel(());
    let global_state = GlobalState::init(shutdown_recv).await;

    info!("Starting Pocket Relay v{}", env::VERSION);

    select! {
        _ = http_server::start_server(global_state.clone()) => { },
        _ = redirector_server::start_server(global_state.clone()) => { },
        _ = start_main_server(global_state) => { },
        _ = signal::ctrl_c() => {
            shutdown_send
                .send(())
                .expect("Failed to send shutdown signal");
        }
    };
}

/// Starts the main server that is currently in use. If MITM mode is
/// enabled then the MITM server takes the place of the main server
///
/// `global_state` The global app state
async fn start_main_server(global_state: GlobalStateArc) {
    let mitm_enabled = env::bool_env(env::MITM_ENABLED);
    if mitm_enabled {
        mitm_server::start_server(global_state).await;
    } else {
        main_server::start_server(global_state).await;
    }
}
