use std::str::FromStr;

use log::LevelFilter;
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

pub fn logging_level() -> LevelFilter {
    const ENV_KEY: &str = "PR_LOG_LEVEL";
    const DEFAULT: LevelFilter = LevelFilter::Info;
    std::env::var(ENV_KEY).map_or(DEFAULT, |value| {
        LevelFilter::from_str(&value).unwrap_or(DEFAULT)
    })
}

/// Initializes the logger
pub fn init_logger(logging_level: LevelFilter, logging_path: String, compress: bool) {
    let pattern = Box::new(PatternEncoder::new("[{d} {h({l})} {M}] {m}{n}"));
    let stdout = ConsoleAppender::builder().encoder(pattern.clone()).build();
    let size_limit = 1024 * 1024; // 1mb max file size before roll
    let size_trigger = SizeTrigger::new(size_limit);
    let window_size = 5;

    let file_pattern = if compress {
        format!("{}/log-{{}}.log.gz", &logging_path)
    } else {
        format!("{}/log-{{}}.log", &logging_path)
    };
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
                .build("pocket_relay", LevelFilter::Info),
        )
        .logger(
            Logger::builder()
                .appenders(["stdout", "file"])
                .additive(false)
                .build("core", logging_level),
        )
        .logger(
            Logger::builder()
                .appenders(["stdout", "file"])
                .additive(false)
                .build("database", logging_level),
        )
        .logger(
            Logger::builder()
                .appenders(["stdout", "file"])
                .additive(false)
                .build("http_server", logging_level),
        )
        .logger(
            Logger::builder()
                .appenders(["stdout", "file"])
                .additive(false)
                .build("main_server", logging_level),
        )
        .logger(
            Logger::builder()
                .appenders(["stdout", "file"])
                .additive(false)
                .build("mitm_server", logging_level),
        )
        .logger(
            Logger::builder()
                .appenders(["stdout", "file"])
                .additive(false)
                .build("redirector_server", logging_level),
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
                .build(LevelFilter::Warn),
        )
        .expect("Failed to create logger config");

    log4rs::init_config(config).expect("Failed to initialize logger");
}
