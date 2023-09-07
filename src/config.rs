use crate::utils::models::Port;
use log::LevelFilter;
use serde::Deserialize;
use std::{env, fs::read_to_string, path::Path};

/// The server version extracted from the Cargo.toml
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct RuntimeConfig {
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
            reverse_proxy: false,
            dashboard: Default::default(),
            menu_message: "<font color='#B2B2B2'>Pocket Relay</font> - <font color='#FFFF66'>Logged as: {n}</font>".to_string(),
            galaxy_at_war: Default::default(),
            logging: LevelFilter::Info,
            retriever: Default::default(),
        }
    }
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
