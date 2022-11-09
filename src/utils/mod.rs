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
    Config,
};
use rand::{thread_rng, Rng};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub mod conv;
pub mod dmap;
pub mod dns;
pub mod hashing;
pub mod ip;

use crate::env;

/// Generates a random alphanumeric token of the provided length
pub fn generate_token(len: usize) -> String {
    thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

/// Returns the current server unix timestamp in seconds.
pub fn server_unix_time() -> u64 {
    let now = SystemTime::now();
    now.duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}

/// Initializes the logger
pub fn init_logger() {
    let logging_level = env::logging_level();
    let logging_path = env::str_env(env::LOGGING_DIR);

    let pattern = Box::new(PatternEncoder::new("[{d} {h({l})} {M}] {m}{n}"));
    let stdout = ConsoleAppender::builder().encoder(pattern.clone()).build();
    let size_limit = 1024 * 1024; // 1mb max file size before roll
    let size_trigger = SizeTrigger::new(size_limit);
    let window_size = 5;

    let file_pattern = format!("{}/log-{{}}.log.gz", &logging_path);
    let latest_path = format!("{}/log.log", &logging_path);

    let fixed_window_roller = FixedWindowRoller::builder()
        .build(&file_pattern, window_size)
        .unwrap();

    let compound_policy =
        CompoundPolicy::new(Box::new(size_trigger), Box::new(fixed_window_roller));
    let file = RollingFileAppender::builder()
        .encoder(pattern)
        .build(&latest_path, Box::new(compound_policy))
        .expect("Unable to create file appender");

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .appender(Appender::builder().build("file", Box::new(file)))
        .logger(
            Logger::builder()
                .appenders(["stdout", "file"])
                .additive(false)
                .build("pocket_relay", logging_level),
        )
        .logger(
            Logger::builder()
                .appenders(["stdout", "file"])
                .additive(false)
                .build("actix_web", logging_level),
        )
        .build(
            Root::builder()
                .appenders(["stdout", "file"])
                .build(log::LevelFilter::Warn),
        )
        .expect("Failed to create logger config");

    log4rs::init_config(config).expect("Failed to initialize logger");
}
