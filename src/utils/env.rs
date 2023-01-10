use log::LevelFilter;
use std::str::FromStr;

use super::models::Port;

pub const REDIRECTOR_PORT: (&str, Port) = ("PR_REDIRECTOR_PORT", 42127);
pub const MAIN_PORT: (&str, Port) = ("PR_MAIN_PORT", 14219);
pub const HTTP_PORT: (&str, Port) = ("PR_HTTP_PORT", 80);
pub const TELEMETRY_PORT: (&str, Port) = ("PR_TELEMETRY_PORT", 9988);
pub const TICKER_PORT: (&str, Port) = ("PR_TICKER_PORT", 8999);

pub const MENU_MESSAGE: (&str, &str) = (
    "PR_MENU_MESSAGE",
    "<font color='#B2B2B2'>Pocket Relay</font> - <font color='#FFFF66'>Logged as: {n}</font>",
);

pub const DATABASE_FILE: (&str, &str) = ("PR_DATABASE_FILE", "data/app.db");
pub const DATABASE_URL: &str = "PR_DATABASE_URL";

pub const GAW_DAILY_DECAY: (&str, f32) = ("PR_GAW_DAILY_DECAY", 0.0);
pub const GAW_PROMOTIONS: (&str, bool) = ("PR_GAW_PROMOTIONS", true);

pub const RETRIEVER: (&str, bool) = ("PR_RETRIEVER", true);

pub const ORIGIN_FETCH: (&str, bool) = ("PR_ORIGIN_FETCH", true);
pub const ORIGIN_FETCH_DATA: (&str, bool) = ("PR_ORIGIN_FETCH_DATA", true);

pub const MITM_ENABLED: (&str, bool) = ("PR_MITM_ENABLED", false);

pub const LOGGING_LEVEL: (&str, LevelFilter) = ("PR_LOG_LEVEL", LevelFilter::Info);
pub const LOGGING_DIR: (&str, &str) = ("PR_LOGGING_DIR", "data/logs");
pub const LOG_COMPRESSION: (&str, bool) = ("PR_LOG_COMPRESSION", true);

pub const API: (&str, bool) = ("PR_API", false);
pub const API_USERNAME: (&str, &str) = ("PR_API_USERNAME", "admin");
pub const API_PASSWORD: (&str, &str) = ("PR_API_PASSWORD", "admin");

#[inline]
pub fn env(pair: (&str, &str)) -> String {
    std::env::var(pair.0).unwrap_or_else(|_| pair.1.to_string())
}

#[inline]
pub fn from_env<F: FromStr>(pair: (&str, F)) -> F {
    if let Ok(value) = std::env::var(pair.0) {
        if let Ok(value) = F::from_str(&value) {
            return value;
        }
    }
    pair.1
}

#[cfg(test)]
mod test {
    use crate::env::from_env;

    #[test]
    fn test_bool() {
        std::env::set_var("TEST", "false");
        assert_eq!(from_env(("TEST", true)), false);

        std::env::set_var("TEST", "False");
        assert_eq!(from_env(("TEST", true)), true);

        std::env::set_var("TEST", "true");
        assert_eq!(from_env(("TEST", false)), true);

        std::env::set_var("TEST", "True");
        assert_eq!(from_env(("TEST", false)), false);

        std::env::set_var("TEST", "12");
        assert_eq!(from_env(("TEST", 0)), 12);
    }
}
