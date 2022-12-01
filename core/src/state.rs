use database::{self, DatabaseConnection, DatabaseType};

use futures::join;
use tokio::{signal, sync::watch};

use crate::{env, game::manager::Games, leaderboard::Leaderboard, retriever::Retriever};

/// Global state that is shared throughout the application this
/// will be unset until the value is initialized then it will be
/// set
pub enum GlobalState {
    Unset,
    Set {
        games: Games,
        db: DatabaseConnection,
        retriever: Option<Retriever>,
        leaderboard: Leaderboard,
        shutdown: watch::Receiver<()>,
    },
}

/// Static global state value
static mut GLOBAL_STATE: GlobalState = GlobalState::Unset;

impl GlobalState {
    /// Initializes the global state storing it in
    /// the option GLOBAL_STATE after everything is
    /// initialized.
    pub async fn init() {
        let (db, retriever) = join!(Self::init_database(), Retriever::new());

        let shutdown = Self::hook_shutdown();
        let games = Games::new();
        let leaderboard = Leaderboard::default();

        unsafe {
            GLOBAL_STATE = GlobalState::Set {
                db,
                games,
                retriever,
                leaderboard,
                shutdown,
            };
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

    /// Obtains a static reference to the database connection
    /// stored on the global state.
    pub fn database() -> &'static DatabaseConnection {
        unsafe {
            match &GLOBAL_STATE {
                GlobalState::Set { db, .. } => db,
                GlobalState::Unset => panic!("Global state not initialized"),
            }
        }
    }

    /// Obtains a static reference to the games manager stored
    /// on the global state
    pub fn games() -> &'static Games {
        unsafe {
            match &GLOBAL_STATE {
                GlobalState::Set { games, .. } => games,
                GlobalState::Unset => panic!("Global state not initialized"),
            }
        }
    }

    /// Obtains a option to the static reference of the retriever
    /// stored on the global state if one exists
    pub fn retriever() -> Option<&'static Retriever> {
        unsafe {
            match &GLOBAL_STATE {
                GlobalState::Set { retriever, .. } => retriever.as_ref(),
                GlobalState::Unset => panic!("Global state not initialized"),
            }
        }
    }

    /// Obtains a option to the static reference of the leaderboard
    /// stored on the global state if one exists
    pub fn leaderboard() -> &'static Leaderboard {
        unsafe {
            match &GLOBAL_STATE {
                GlobalState::Set { leaderboard, .. } => leaderboard,
                GlobalState::Unset => panic!("Global state not initialized"),
            }
        }
    }

    /// Obtains a clone of the shutdown receiever from the global state
    pub fn shutdown() -> watch::Receiver<()> {
        unsafe {
            match &GLOBAL_STATE {
                GlobalState::Set { shutdown, .. } => shutdown.clone(),
                GlobalState::Unset => panic!("Global state not initialized"),
            }
        }
    }
}
