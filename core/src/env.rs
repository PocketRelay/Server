pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The external host environment variable
pub const EXT_HOST: (&str, &str) = ("PR_EXT_HOST", "gosredirector.ea.com");

pub const REDIRECTOR_PORT: (&str, u16) = ("PR_REDIRECTOR_PORT", 42127);
pub const MAIN_PORT: (&str, u16) = ("PR_MAIN_PORT", 14219);
pub const HTTP_PORT: (&str, u16) = ("PR_HTTP_PORT", 80);
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
pub const LOGGING_DIR: (&str, &str) = ("PR_LOGGING_DIR", "data/logs");

#[inline]
pub fn str_env(pair: (&str, &str)) -> String {
    std::env::var(pair.0).unwrap_or_else(|_| pair.1.to_string())
}

#[inline]
pub fn u16_env(pair: (&str, u16)) -> u16 {
    std::env::var(pair.0).map_or(pair.1, |value| value.parse::<u16>().unwrap_or(pair.1))
}

#[inline]
pub fn f32_env(pair: (&str, f32)) -> f32 {
    std::env::var(pair.0).map_or(pair.1, |value| value.parse::<f32>().unwrap_or(pair.1))
}

#[inline]
pub fn bool_env(pair: (&str, bool)) -> bool {
    std::env::var(pair.0).map_or(pair.1, |value| {
        value.to_lowercase().parse::<bool>().unwrap_or(pair.1)
    })
}
