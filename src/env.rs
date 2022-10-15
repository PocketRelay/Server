use std::str::FromStr;
use log::LevelFilter;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const ADDRESS: &str = "gosredirector.ea.com";

pub fn main_port() -> u16 {
    const ENV_KEY: &str = "PR_MAIN_PORT";
    const DEFAULT: u16 = 14219;

    std::env::var(ENV_KEY)
        .map_or(DEFAULT, |value| value.parse::<u16>().unwrap_or(DEFAULT))
}

pub fn http_port() -> u16 {
    const ENV_KEY: &str = "PR_HTTP_PORT";
    const DEFAULT: u16 = 80;

    std::env::var(ENV_KEY)
        .map_or(DEFAULT, |value| value.parse::<u16>().unwrap_or(DEFAULT))
}

pub fn logging_level() -> LevelFilter {
    const ENV_KEY: &str = "PR_LOG_LEVEL";
    const DEFAULT: LevelFilter = LevelFilter::Info;
    std::env::var(ENV_KEY)
        .map_or(DEFAULT, |value| LevelFilter::from_str(&value).unwrap_or(DEFAULT))
}

#[allow(unused)]
pub fn menu_message() -> String {
    const ENV_KEY: &str = "PR_MENU_MESSAGE";
    const DEFAULT: &str = "<font color='#B2B2B2'>Pocket Relay</font> - <font color='#FFFF66'>Logged as: {n}</font>";
    std::env::var(ENV_KEY).unwrap_or_else(|_|DEFAULT.to_string())
}

#[allow(unused)]
pub fn database_file() -> &'static str {
    return "app.db";
}