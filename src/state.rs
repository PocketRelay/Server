use crate::{
    config::{Config, RuntimeConfig, ServicesConfig},
    database::{self, DatabaseConnection},
    services::Services,
};
use tokio::join;

/// The server version extracted from the Cargo.toml
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Global state that is shared throughout the application this
/// will be unset until the value is initialized then it will be
/// set
pub struct GlobalState {
    /// Connection to the database
    pub db: DatabaseConnection,
    /// Global services
    pub services: Services,
    /// Runtime global configuration
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
        let dashboard_config = config.dashboard;

        // Config data passed onto the services
        let services_config = ServicesConfig {
            retriever: config.retriever,
        };

        // Config data persisted to runtime
        let runtime_config = RuntimeConfig {
            galaxy_at_war: config.galaxy_at_war,
            menu_message: config.menu_message,
        };

        let (db, services) = join!(
            database::init(dashboard_config),
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
