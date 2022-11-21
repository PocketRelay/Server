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

static mut GLOBAL_STATE: Option<GlobalState> = None;

impl GlobalState {
    /// Initializes the global state with the provided shutdown
    /// reciever and returns it wrapped in an Arc and the reciever
    pub async fn init() {
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

        unsafe {
            GLOBAL_STATE = Some(global_state);
        }
    }

    pub fn get() -> &'static Self {
        unsafe { GLOBAL_STATE.as_ref().expect("Global state was missing") }
    }

    pub fn database() -> &'static Database {
        &Self::get().db
    }

    pub fn games() -> &'static Games {
        &Self::get().games
    }

    pub fn retriever() -> Option<&'static Retriever> {
        Self::get().retriever.as_ref()
    }

    pub fn shutdown() -> watch::Receiver<()> {
        Self::get().shutdown.clone()
    }
}
