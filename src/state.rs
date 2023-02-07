use crate::{env, services::Services};
use database::{self, DatabaseConnection, DatabaseType};
use tokio::join;

/// Global state that is shared throughout the application this
/// will be unset until the value is initialized then it will be
/// set
pub struct GlobalState {
    pub db: DatabaseConnection,
    pub services: Services,
}

/// Static global state value
static mut GLOBAL_STATE: Option<GlobalState> = None;

impl GlobalState {
    /// Initializes the global state updating the value stored in
    /// GLOBAL_STATE with a new set state. This function MUST be
    /// called before this state is accessed or else the app will
    /// panic and must not be called more than once.
    pub async fn init() {
        let (db, services) = join!(Self::init_database(), Services::init());
        unsafe {
            GLOBAL_STATE = Some(GlobalState { db, services });
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

    /// Obtains a database connection by cloning the global
    /// database pool
    pub fn database() -> DatabaseConnection {
        unsafe {
            match &GLOBAL_STATE {
                Some(value) => value.db.clone(),
                None => panic!("Global state not initialized"),
            }
        }
    }

    pub fn services() -> &'static Services {
        unsafe {
            match &GLOBAL_STATE {
                Some(value) => &value.services,
                None => panic!("Global state not initialized"),
            }
        }
    }
}
