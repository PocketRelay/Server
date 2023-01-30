use crate::{
    env, game::manager::Games, leaderboard::Leaderboard, retriever::Retriever, utils::jwt::Jwt,
};
use database::{self, DatabaseConnection, DatabaseType};
use tokio::join;

/// Global state that is shared throughout the application this
/// will be unset until the value is initialized then it will be
/// set
pub struct GlobalState {
    pub games: Games,
    pub db: DatabaseConnection,
    pub retriever: Option<Retriever>,
    pub leaderboard: Leaderboard,
    pub jwt: Jwt,
}

/// Static global state value
static mut GLOBAL_STATE: Option<GlobalState> = None;

impl GlobalState {
    /// Initializes the global state updating the value stored in
    /// GLOBAL_STATE with a new set state. This function MUST be
    /// called before this state is accessed or else the app will
    /// panic and must not be called more than once.
    pub async fn init() {
        let (db, retriever, jwt) = join!(Self::init_database(), Retriever::new(), Jwt::new());

        let games: Games = Games::default();
        let leaderboard: Leaderboard = Leaderboard::default();

        unsafe {
            GLOBAL_STATE = Some(GlobalState {
                db,
                games,
                retriever,
                leaderboard,
                jwt,
            });
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
                Some(value) => &value.db,
                None => panic!("Global state not initialized"),
            }
        }
    }

    /// Obtains a static reference to the games manager stored
    /// on the global state
    pub fn games() -> &'static Games {
        unsafe {
            match &GLOBAL_STATE {
                Some(value) => &value.games,
                None => panic!("Global state not initialized"),
            }
        }
    }

    /// Obtains a option to the static reference of the retriever
    /// stored on the global state if one exists
    pub fn retriever() -> Option<&'static Retriever> {
        unsafe {
            match &GLOBAL_STATE {
                Some(value) => value.retriever.as_ref(),
                None => panic!("Global state not initialized"),
            }
        }
    }

    /// Obtains a static reference to the leaderboard
    /// stored on the global state if one exists
    pub fn leaderboard() -> &'static Leaderboard {
        unsafe {
            match &GLOBAL_STATE {
                Some(value) => &value.leaderboard,
                None => panic!("Global state not initialized"),
            }
        }
    }

    /// Obtains a static reference to the jwt sate
    /// stored on the global state if one exists
    pub fn jwt() -> &'static Jwt {
        unsafe {
            match &GLOBAL_STATE {
                Some(value) => &value.jwt,
                None => panic!("Global state not initialized"),
            }
        }
    }
}
