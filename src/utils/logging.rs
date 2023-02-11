use crate::{env, utils::net::public_address};
use log::{info, LevelFilter};
use log4rs::{
    append::{
        console::ConsoleAppender,
        rolling_file::{
            policy::compound::{
                roll::fixed_window::FixedWindowRoller, trigger::size::SizeTrigger, CompoundPolicy,
            },
            RollingFileAppender,
        },
    },
    config::{Appender, Logger, Root},
    encode::pattern::PatternEncoder,
    init_config, Config,
};

/// The pattern to use when logging
const LOGGING_PATTERN: &str = "[{d} {h({l})} {M}] {m}{n}";
/// Max logging file size before rolling over to the next log file. (5mb)
const LOGGING_MAX_SIZE: u64 = 1024 * 1024 * 5;
/// The max number of logging files to keep before deleting
const LOGGING_MAX_FILES: u32 = 8;

/// Setup function for setting up the Log4rs logging configuring it
/// for all the different modules and and setting up file and stdout logging
pub fn setup() {
    // Configuration from environment
    let logging_level = env::from_env(env::LOGGING_LEVEL);
    if logging_level == LevelFilter::Off {
        // Don't initialize logger at all if logging is disabled
        return;
    }
    let logging_path = env::env(env::LOGGING_DIR);

    let pattern = Box::new(PatternEncoder::new(LOGGING_PATTERN));
    let size_trigger = SizeTrigger::new(LOGGING_MAX_SIZE);

    let file_pattern = format!("{}/log-{{}}.log", &logging_path);

    let latest_path = format!("{}/log.log", &logging_path);

    let fixed_window_roller = FixedWindowRoller::builder()
        .build(&file_pattern, LOGGING_MAX_FILES)
        .expect("Unable to create fixed window log roller");

    let compound_policy =
        CompoundPolicy::new(Box::new(size_trigger), Box::new(fixed_window_roller));

    let stdout_appender = ConsoleAppender::builder().encoder(pattern.clone()).build();

    let file_appender = RollingFileAppender::builder()
        .encoder(pattern)
        .build(latest_path, Box::new(compound_policy))
        .expect("Unable to create logging file appender");

    const APPENDERS: [&str; 2] = ["stdout", "file"];

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout_appender)))
        .appender(Appender::builder().build("file", Box::new(file_appender)))
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
}

/// Prints a list of possible urls that can be used to connect to
/// this Pocket relay server
pub async fn log_connection_urls() {
    let http_port = env::from_env(env::HTTP_PORT);
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
