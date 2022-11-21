use database::{self, DatabaseConnection, DatabaseType};

use futures::join;
use tokio::{signal, sync::watch};

use crate::{env, game::manager::Games, retriever::Retriever};

/// Global state that is shared throughout the application
pub struct GlobalState {
    pub games: Games,
    pub db: DatabaseConnection,
    pub retriever: Option<Retriever>,
    pub shutdown: watch::Receiver<()>,
}

/// Static option for storing the Global state after it has been
/// initialized. The state should always be initialized be accessing
static mut GLOBAL_STATE: Option<GlobalState> = None;

impl GlobalState {
    /// Initializes the global state storing it in
    /// the option GLOBAL_STATE after everything is
    /// initialized.
    pub async fn init() {
        let (db, retriever) = join!(Self::init_database(), Retriever::new());

        let shutdown = Self::hook_shutdown();
        let games = Games::new();

        let global_state = GlobalState {
            db,
            games,
            retriever,
            shutdown,
        };

        unsafe {
            GLOBAL_STATE = Some(global_state);
        }
    }

    /// Spawns a tokio task which waits for the CTRL C signal
    /// and creates a channel returning the receiver for the
    /// channel.
    fn hook_shutdown() -> watch::Receiver<()> {
        // Channel for safely shutdown
        let (shutdown_send, shutdown_recv) = watch::channel(());
        // Spawn a handler for safe shutdown
        tokio::spawn(async move {
            signal::ctrl_c().await.ok();
            shutdown_send.send(()).ok();
        });
        shutdown_recv
    }

    /// Initializes the connection with the database using the url or file
    /// from the environment variables
    async fn init_database() -> DatabaseConnection {
        let ty = if cfg!(feature = "database-sqlite") {
            let file = env::env(env::DATABASE_FILE);
            DatabaseType::Sqlite(file)
        } else {
            let url = std::env::var(env::DATABASE_URL).expect(
                "Environment PR_DATABASE_URL is required for non-sqlite versions of Pocket Relay",
            );
            DatabaseType::MySQL(url)
        };
        database::connect(ty).await
    }

    /// Obtains a static reference to the global state panicing if
    /// the global state is not yet initialized.
    pub fn get() -> &'static Self {
        unsafe { GLOBAL_STATE.as_ref().expect("Global state was missing") }
    }

    /// Obtains a static reference to the database connection
    /// stored on the global state.
    pub fn database() -> &'static DatabaseConnection {
        &Self::get().db
    }

    /// Obtains a static reference to the games manager stored
    /// on the global state
    pub fn games() -> &'static Games {
        &Self::get().games
    }

    /// Obtains a option to the static reference of the retriever
    /// stored on the global state if one exists
    pub fn retriever() -> Option<&'static Retriever> {
        Self::get().retriever.as_ref()
    }

    /// Obtains a clone of the shutdown receiever from the global state
    pub fn shutdown() -> watch::Receiver<()> {
        Self::get().shutdown.clone()
    }
}
