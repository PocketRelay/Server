use std::{env, path::Path};

use log::LevelFilter;
use serde::Deserialize;
use tokio::fs::read_to_string;

use crate::utils::models::Port;

pub struct RuntimeConfig {
    pub ports: PortsConfig,
    pub galaxy_at_war: GalaxyAtWarConfig,
    pub menu_message: String,
}

const CONFIG_ENV_KEY: &str = "PR_CONFIG_JSON";

pub async fn load_config() -> Option<Config> {
    let file = Path::new("config.json");
    if !file.exists() {
        return None;
    }

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

    let data = match read_to_string(file).await {
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

pub struct ServicesConfig {
    pub retriever: RetrieverConfig,
}

#[derive(Deserialize)]
#[serde(default)]
pub struct Config {
    pub ports: PortsConfig,
    pub dashboard: DashboardConfig,
    pub menu_message: String,
    pub galaxy_at_war: GalaxyAtWarConfig,
    pub logging: LevelFilter,
    pub retriever: RetrieverConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ports: Default::default(),
            dashboard: Default::default(),
            menu_message:  "<font color='#B2B2B2'>Pocket Relay</font> - <font color='#FFFF66'>Logged as: {n}</font>".to_string(),
            galaxy_at_war: Default::default(),
            logging: LevelFilter::Info,
            retriever: Default::default(),
        }
    }
}

/// Server ports configuration data
#[derive(Deserialize)]
#[serde(default)]
pub struct PortsConfig {
    pub redirector: Port,
    pub main: Port,
    pub telemetry: Port,
    pub qos: Port,
    pub http: Port,
}

impl Default for PortsConfig {
    fn default() -> Self {
        Self {
            redirector: 42127,
            main: 42128,
            telemetry: 42129,
            qos: 42130,
            http: 80,
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
