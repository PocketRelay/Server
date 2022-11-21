use std::sync::Arc;

use database::Database;
use tokio::{signal, sync::watch};

use crate::{env, game::manager::Games, retriever::Retriever};

/// Global state that is shared throughout the application
pub struct GlobalState {
    pub games: Games,
    pub db: Database,
    pub retriever: Option<Retriever>,
    pub shutdown: watch::Receiver<()>,
}

pub type GlobalStateArc = Arc<GlobalState>;

impl GlobalState {
    /// Initializes the global state with the provided shutdown
    /// reciever and returns it wrapped in an Arc and the reciever
    pub async fn init() -> Arc<Self> {
        let db = {
            if cfg!(feature = "database-sqlite") {
                let file = env::env(env::DATABASE_FILE);
                Database::connect_sqlite(file).await
            } else {
                let url = std::env::var(env::DATABASE_URL)
                    .expect("Environment PR_DATABASE_URL is required for non-sqlite versions of Pocket Relay");
                Database::connect_url(url).await
            }
        };

        let games = Games::new();
        let retriever = Retriever::new().await;

        // Channel for safely shutdown
        let (shutdown_send, shutdown_recv) = watch::channel(());

        // Spawn a handler for safe shutdown
        tokio::spawn(async move {
            signal::ctrl_c().await.ok();
            shutdown_send.send(()).ok();
        });

        let global_state = GlobalState {
            db,
            games,
            retriever,
            shutdown: shutdown_recv,
        };

        Arc::new(global_state)
    }
}
