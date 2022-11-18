use log::info;
use tokio::{select, signal, sync::watch};

use core::{env, GlobalState};

use http_server;
use main_server;
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
        _ = main_server::start_server(global_state) => { },
        _ = signal::ctrl_c() => {
            shutdown_send
                .send(())
                .expect("Failed to send shutdown signal");
        }
    };
}
