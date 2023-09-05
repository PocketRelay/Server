use crate::{
    config::{Config, RuntimeConfig, ServicesConfig},
    database::{self, DatabaseConnection},
    services::Services,
    session::{self, router::Router},
    utils::logging,
};
use tokio::join;

/// The server version extracted from the Cargo.toml
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Global state that is shared throughout the application this
/// will be unset until the value is initialized then it will be
/// set
pub struct App {
    /// Connection to the database
    pub db: DatabaseConnection,
    /// Global services
    pub services: Services,
    /// Runtime global configuration
    pub config: RuntimeConfig,
    /// Global session router
    pub router: Router,
}

/// Static global state value
static mut GLOBAL_STATE: Option<App> = None;

impl App {
    /// Initializes the global state updating the value stored in
    /// GLOBAL_STATE with a new set state. This function MUST be
    /// called before this state is accessed or else the app will
    /// panic and must not be called more than once.
    pub async fn init(config: Config) {
        // Config data passed onto the services
        let services_config = ServicesConfig {
            retriever: config.retriever,
        };

        // Create menu message
        let menu_message = {
            // Message with server version variable replaced
            let mut message: String = config.menu_message.replace("{v}", VERSION);

            // Line terminator for the end of the message
            message.push(char::from(0x0A));
            message
        };

        // Config data persisted to runtime
        let runtime_config = RuntimeConfig {
            reverse_proxy: config.reverse_proxy,
            galaxy_at_war: config.galaxy_at_war,
            menu_message,
            dashboard: config.dashboard,
        };

        let (db, services, _) = join!(
            // Initialize the database
            database::init(&runtime_config),
            // Initialize the services
            Services::init(services_config),
            // Display the connection urls message
            logging::log_connection_urls(config.port)
        );

        // Initialize session router
        let router = session::routes::router();

        unsafe {
            GLOBAL_STATE = Some(App {
                db,
                services,
                config: runtime_config,
                router,
            });
        }
    }

    /// Obtains a static reference to the session router
    pub fn router() -> &'static Router {
        match unsafe { &GLOBAL_STATE } {
            Some(value) => &value.router,
            None => panic!("Global state not initialized"),
        }
    }

    /// Obtains a database connection by cloning the global
    /// database pool
    pub fn database() -> &'static DatabaseConnection {
        match unsafe { &GLOBAL_STATE } {
            Some(value) => &value.db,
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
