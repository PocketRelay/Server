use log::LevelFilter;
use serde::Deserialize;
use std::{env, fs::read_to_string, path::Path};

use crate::session::models::Port;

/// The server version extracted from the Cargo.toml
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Default)]
pub struct RuntimeConfig {
    pub qos: QosServerConfig,
    pub reverse_proxy: bool,
    pub galaxy_at_war: GalaxyAtWarConfig,
    pub menu_message: String,
    pub dashboard: DashboardConfig,
}

/// Environment variable key to load the config from
const CONFIG_ENV_KEY: &str = "PR_CONFIG_JSON";

pub fn load_config() -> Option<Config> {
    // Attempt to load the config from the env
    if let Ok(env) = env::var(CONFIG_ENV_KEY) {
        let config: Config = match serde_json::from_str(&env) {
            Ok(value) => value,
            Err(err) => {
                eprintln!("Failed to load env config (Using default): {:?}", err);
                return None;
            }
        };
        return Some(config);
    }

    // Attempt to load the config from disk
    let file = Path::new("config.json");
    if !file.exists() {
        return None;
    }

    let data = match read_to_string(file) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Failed to load config file (Using defaults): {:?}", err);
            return None;
        }
    };

    let config: Config = match serde_json::from_str(&data) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Failed to load config file (Using default): {:?}", err);
            return None;
        }
    };

    Some(config)
}

#[derive(Deserialize)]
#[serde(default)]
pub struct Config {
    pub port: Port,
    pub qos: QosServerConfig,
    pub reverse_proxy: bool,
    pub dashboard: DashboardConfig,
    pub menu_message: String,
    pub galaxy_at_war: GalaxyAtWarConfig,
    pub logging: LevelFilter,
    pub retriever: RetrieverConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 80,
            qos: QosServerConfig::default(),
            reverse_proxy: false,
            dashboard: Default::default(),
            menu_message: "<font color='#B2B2B2'>Pocket Relay</font> - <font color='#FFFF66'>Logged as: {n}</font>".to_string(),
            galaxy_at_war: Default::default(),
            logging: LevelFilter::Info,
            retriever: Default::default(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum QosServerConfig {
    /// Use the official QoS server
    Official,
    /// Use the local QoS server (might cause issues)
    #[default]
    Local,
    /// Use a custom QoS server
    Custom { host: String, port: u16 },
}

#[derive(Deserialize)]
#[serde(default)]
pub struct GalaxyAtWarConfig {
    pub decay: f32,
    pub promotions: bool,
}

impl Default for GalaxyAtWarConfig {
    fn default() -> Self {
        Self {
            decay: 0.0,
            promotions: true,
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct DashboardConfig {
    pub super_email: Option<String>,
    pub super_password: Option<String>,
    pub disable_registration: bool,
}

#[derive(Deserialize)]
#[serde(default)]
pub struct RetrieverConfig {
    pub enabled: bool,
    pub origin_fetch: bool,
    pub origin_fetch_data: bool,
}

impl Default for RetrieverConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            origin_fetch: true,
            origin_fetch_data: true,
        }
    }
}
