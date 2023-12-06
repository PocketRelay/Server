use futures_util::TryFutureExt;
use log::{info, LevelFilter};
use log4rs::{
    append::{console::ConsoleAppender, file::FileAppender},
    config::{Appender, Logger, Root},
    encode::pattern::PatternEncoder,
    init_config, Config,
};
use std::net::Ipv4Addr;

/// The pattern to use when logging
const LOGGING_PATTERN: &str = "[{d} {h({l})} {M}] {m}{n}";

/// Log file name
pub const LOG_FILE_NAME: &str = "data/server.log";

/// Setup function for setting up the Log4rs logging configuring it
/// for all the different modules and and setting up file and stdout logging
pub fn setup(logging_level: LevelFilter) {
    if logging_level == LevelFilter::Off {
        // Don't initialize logger at all if logging is disabled
        return;
    }

    // Create logging appenders
    let pattern = Box::new(PatternEncoder::new(LOGGING_PATTERN));
    let console = Box::new(ConsoleAppender::builder().encoder(pattern.clone()).build());
    let file = Box::new(
        FileAppender::builder()
            .encoder(pattern)
            .build(LOG_FILE_NAME)
            .expect("Unable to create logging file appender"),
    );

    const APPENDERS: [&str; 2] = ["stdout", "file"];

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", console))
        .appender(Appender::builder().build("file", file))
        .logger(
            Logger::builder()
                .appenders(APPENDERS)
                .additive(false)
                .build("pocket_relay", logging_level),
        )
        .build(
            Root::builder()
                .appenders(APPENDERS)
                .build(LevelFilter::Warn),
        )
        .expect("Failed to create logging config");

    init_config(config).expect("Unable to initialize logger");

    // Include panics in logging
    log_panics::init();
}

/// Prints a list of possible urls that can be used to connect to
/// this Pocket relay server
pub async fn log_connection_urls(http_port: u16) {
    let mut output = String::new();
    if let Ok(local_address) = local_ip_address::local_ip() {
        output.push_str("LAN: ");
        output.push_str(&local_address.to_string());
        if http_port != 80 {
            output.push(':');
            output.push_str(&http_port.to_string());
        }
    }
    if let Some(public_address) = public_address().await {
        if !output.is_empty() {
            output.push_str(", ");
        }
        output.push_str(&format!("WAN: {}", public_address));
        if http_port != 80 {
            output.push(':');
            output.push_str(&http_port.to_string());
        }
    }

    if !output.is_empty() {
        output.push_str(", ");
    }

    output.push_str("LOCAL: 127.0.0.1");
    if http_port != 80 {
        output.push(':');
        output.push_str(&http_port.to_string());
    }

    info!("Connection URLS ({output})");
}

/// Retrieves the public address of the server either using the cached
/// value if its not expired or fetching the new value from the API using
/// `fetch_public_addr`
pub async fn public_address() -> Option<Ipv4Addr> {
    // API addresses for IP lookup
    let addresses = ["https://api.ipify.org/", "https://ipv4.icanhazip.com/"];

    // Try all addresses using the first valid value
    for address in addresses {
        let addr = match reqwest::get(address)
            // Read the response as text
            .and_then(reqwest::Response::text)
            .await
        {
            Ok(value) => value,
            Err(_) => continue,
        };

        let addr = addr
            // Trim whitespace and new lines
            .trim_matches(|c: char| c == '\n' || c.is_whitespace())
            // Attempt to parse as an IPv4 address
            .parse::<Ipv4Addr>();

        if let Ok(parsed) = addr {
            return Some(parsed);
        }
    }

    None
}
