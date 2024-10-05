use log::LevelFilter;
use serde::Deserialize;
use std::{
    env,
    fs::read_to_string,
    net::{IpAddr, Ipv4Addr},
    path::Path,
};

use crate::session::models::Port;

/// The server version extracted from the Cargo.toml
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Config variables that are required to always exist during
/// runtime for various tasks
#[derive(Default)]
pub struct RuntimeConfig {
    pub qos: QosServerConfig,
    pub reverse_proxy: bool,
    pub galaxy_at_war: GalaxyAtWarConfig,
    pub menu_message: String,
    pub dashboard: DashboardConfig,
    pub tunnel: TunnelConfig,
    pub udp_tunnel: UdpTunnelConfig,
    pub api: APIConfig,
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
    pub host: IpAddr,
    pub port: Port,
    pub qos: QosServerConfig,
    pub reverse_proxy: bool,
    pub dashboard: DashboardConfig,
    pub menu_message: String,
    pub galaxy_at_war: GalaxyAtWarConfig,
    pub logging: LevelFilter,
    pub retriever: RetrieverConfig,
    pub tunnel: TunnelConfig,
    pub udp_tunnel: UdpTunnelConfig,
    pub api: APIConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: 80,
            qos: QosServerConfig::default(),
            reverse_proxy: false,
            dashboard: Default::default(),
            menu_message: "<font color='#B2B2B2'>Pocket Relay</font> - <font color='#FFFF66'>Logged as: {n}</font>".to_string(),
            galaxy_at_war: Default::default(),
            logging: LevelFilter::Info,
            retriever: Default::default(),
            tunnel: Default::default(),
            udp_tunnel: Default::default(),
            api: Default::default()
        }
    }
}

/// Configuration for how the server should use tunneling
///
/// This option applies to both the HTTP and UDP tunnels
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TunnelConfig {
    /// Only tunnel players with non "Open" NAT types if the QoS
    /// server is set to [`QosServerConfig::Disabled`] this is
    /// equivalent to [`TunnelConfig::Always`]
    #[default]
    Stricter,
    /// Always tunnel connections through the server regardless
    /// of NAT type
    Always,
    /// Never tunnel connections through the server
    Disabled,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct UdpTunnelConfig {
    /// Port to bind the UDP tunnel socket to, the socket is bound
    /// using the same host as the server
    pub port: Port,

    /// External facing port, only needed when the port visible to users
    /// is different to [UdpTunnelConfig::port]
    ///
    /// For cases such as different exposed port in docker or usage behind
    /// a reverse proxy such as NGINX
    pub external_port: Option<Port>,

    /// Optionally choose to disable the tunnel if you don't intend to use it
    /// default value is true
    pub enabled: bool,
}

impl Default for UdpTunnelConfig {
    fn default() -> Self {
        Self {
            port: 9032,
            external_port: None,
            enabled: true,
        }
    }
}

impl UdpTunnelConfig {
    /// Get the port the exposed to the clients for the UDP
    /// tunnel. This is [None] if the tunnel is disabled. Otherwise
    /// its [UdpTunnelConfig::external_port] if set otherwise its
    /// [UdpTunnelConfig::port]
    pub fn get_exposed_port(&self) -> Option<Port> {
        if !self.enabled {
            return None;
        }

        Some(self.external_port.unwrap_or(self.port))
    }
}

/// Configuration for the server QoS setup
#[derive(Debug, Default, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum QosServerConfig {
    /// Use the official QoS server
    Official,
    /// Use the local QoS server (might cause issues with WAN)
    #[default]
    Local,
    /// Use a custom QoS server
    Custom { host: String, port: u16 },
    /// Disable the QoS server (Public IP *wont* be resolved)
    Disabled,
    /// Configuration to use when using hamachi
    Hamachi {
        /// The address of the host computer (Required for 127.0.0.1 resolution)
        host: Ipv4Addr,
    },
}

#[derive(Deserialize)]
#[serde(default)]
pub struct APIConfig {
    /// Allow games data to be requested from the API without auth
    pub public_games: bool,
    /// Hide players from API response when no auth is provided
    pub public_games_hide_players: bool,
}

impl Default for APIConfig {
    fn default() -> Self {
        Self {
            public_games: false,
            public_games_hide_players: true,
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
#[serde(default)]
pub struct DashboardConfig {
    pub super_email: Option<String>,
    pub super_password: Option<String>,
    pub disable_registration: bool,
}

impl DashboardConfig {
    pub fn is_super_email(&self, email: &str) -> bool {
        self.super_email
            .as_ref()
            .is_some_and(|value| !value.is_empty() && value.eq(email))
    }
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
