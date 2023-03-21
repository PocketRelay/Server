use crate::{
    config::{Config, DashboardConfig, RuntimeConfig, ServicesConfig},
    services::Services,
    utils::hashing::hash_password,
};
use database::{self, DatabaseConnection, Player, PlayerRole};
use log::{error, info};
use tokio::join;

/// The server version extracted from the Cargo.toml
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The external address of the server. This address is whats used in
/// the system hosts file as a redirect so theres no need to use any
/// other address.
pub const EXTERNAL_HOST: &str = "gosredirector.ea.com";

/// Global state that is shared throughout the application this
/// will be unset until the value is initialized then it will be
/// set
pub struct GlobalState {
    pub db: DatabaseConnection,
    pub services: Services,
    pub config: RuntimeConfig,
}

/// Static global state value
static mut GLOBAL_STATE: Option<GlobalState> = None;

impl GlobalState {
    /// Initializes the global state updating the value stored in
    /// GLOBAL_STATE with a new set state. This function MUST be
    /// called before this state is accessed or else the app will
    /// panic and must not be called more than once.
    pub async fn init(config: Config) {
        let admin_email = config.dashboard;

        // Config data passed onto the services
        let services_config = ServicesConfig {
            retriever: config.retriever,
        };

        // Config data persisted to runtime
        let runtime_config = RuntimeConfig {
            port: config.port,
            galaxy_at_war: config.galaxy_at_war,
            menu_message: config.menu_message,
        };

        let (db, services) = join!(
            Self::init_database(admin_email),
            Services::init(services_config)
        );

        unsafe {
            GLOBAL_STATE = Some(GlobalState {
                db,
                services,
                config: runtime_config,
            });
        }
    }

    /// Initializes the connection with the database using the url or file
    /// from the environment variables
    async fn init_database(config: DashboardConfig) -> DatabaseConnection {
        let db = database::init().await;
        info!("Connected to database..");
        Self::init_database_admin(&db, config).await;

        db
    }

    /// Initializes the database super admin account using the
    /// admin email stored within the environment variables if
    /// one is present
    ///
    /// `db` The database connection
    async fn init_database_admin(db: &DatabaseConnection, config: DashboardConfig) {
        let admin_email = match config.super_email {
            Some(value) => value,
            None => return,
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

        let player = match player.set_role(db, PlayerRole::SuperAdmin).await {
            Ok(value) => value,
            Err(err) => {
                error!("Failed to assign super admin role: {:?}", err);
                return;
            }
        };

        if let Some(password) = config.super_password {
            let password = hash_password(&password).expect("Failed to hash super user password");
            let matches = match &player.password {
                Some(value) => value.eq(&password),
                None => false,
            };

            if !matches {
                if let Err(err) = player.set_password(db, password).await {
                    error!("Failed to set super admin password: {:?}", err)
                } else {
                    info!("Updated super admin password")
                }
            }
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

    /// Obtains a static reference to the runtime config
    pub fn config() -> &'static RuntimeConfig {
        match unsafe { &GLOBAL_STATE } {
            Some(value) => &value.config,
            None => panic!("Global state not initialized"),
        }
    }
}
