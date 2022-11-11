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

/// Initializes the logger
pub fn init_logger(logging_level: LevelFilter, logging_path: String) {
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
                .build(LevelFilter::Warn),
        )
        .expect("Failed to create logger config");

    log4rs::init_config(config).expect("Failed to initialize logger");
}
