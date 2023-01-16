use crate::{env, game::manager::Games, leaderboard::Leaderboard, retriever::Retriever};
use database::{self, DatabaseConnection, DatabaseType};
use tokio::join;

/// Global state that is shared throughout the application this
/// will be unset until the value is initialized then it will be
/// set
pub enum GlobalState {
    // Default state before the application is initialized
    Unset,
    // Actual application state
    Set {
        games: Games,
        db: DatabaseConnection,
        retriever: Option<Retriever>,
        leaderboard: Leaderboard,
    },
}

/// Static global state value
static mut GLOBAL_STATE: GlobalState = GlobalState::Unset;

impl GlobalState {
    /// Initializes the global state updating the value stored in
    /// GLOBAL_STATE with a new set state. This function MUST be
    /// called before this state is accessed or else the app will
    /// panic and must not be called more than once.
    pub async fn init() {
        let (db, retriever) = join!(Self::init_database(), Retriever::new());

        let games: Games = Games::default();
        let leaderboard: Leaderboard = Leaderboard::default();

        unsafe {
            GLOBAL_STATE = GlobalState::Set {
                db,
                games,
                retriever,
                leaderboard,
            };
        }
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

    /// Obtains a static reference to the leaderboard
    /// stored on the global state if one exists
    pub fn leaderboard() -> &'static Leaderboard {
        unsafe {
            match &GLOBAL_STATE {
                GlobalState::Set { leaderboard, .. } => leaderboard,
                GlobalState::Unset => panic!("Global state not initialized"),
            }
        }
    }
}
