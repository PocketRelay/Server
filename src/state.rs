use crate::{env, services::Services};
use database::{self, DatabaseConnection, Player, PlayerRole};
use log::{error, info};
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
        let db = database::init().await;
        info!("Connected to database..");
        Self::init_database_admin(&db).await;
        db
    }

    /// Initializes the database super admin account using the
    /// admin email stored within the environment variables if
    /// one is present
    ///
    /// `db` The database connection
    async fn init_database_admin(db: &DatabaseConnection) {
        let admin_email = match std::env::var(env::SUPER_ADMIN_EMAIL) {
            Ok(value) => value,
            Err(_) => {
                info!(
                    "{} not set. Will not assign super admin to any accounts.",
                    env::SUPER_ADMIN_EMAIL
                );
                return;
            }
        };

        let player = match Player::by_email(db, &admin_email).await {
            // Player exists
            Ok(Some(value)) => value,
            // Player doesn't exist yet
            Ok(None) => return,
            // Encountered an error
            Err(err) => {
                error!("Failed to find player to provide super admin: {:?}", err);
                return;
            }
        };

        if let Err(err) = player.set_role(db, PlayerRole::SuperAdmin).await {
            error!("Failed to assign super admin role: {:?}", err);
        }
    }

    /// Obtains a database connection by cloning the global
    /// database pool
    pub fn database() -> DatabaseConnection {
        match unsafe { &GLOBAL_STATE } {
            Some(value) => value.db.clone(),
            None => panic!("Global state not initialized"),
        }
    }

    /// Obtains a static reference to the services
    pub fn services() -> &'static Services {
        match unsafe { &GLOBAL_STATE } {
            Some(value) => &value.services,
            None => panic!("Global state not initialized"),
        }
    }
}
