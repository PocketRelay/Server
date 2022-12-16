use crate::env;
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
    init_config, Config,
};

/// The pattern to use when logging
const LOGGING_PATTERN: &str = "[{d} {h({l})} {M}] {m}{n}";
/// Max logging file size before rolling over to the next log file. (5mb)
const LOGGING_MAX_SIZE: u64 = 1024 * 1024 * 5;
/// The max number of logging files to keep before deleting
const LOGGING_MAX_FILES: u32 = 8;
/// The modules to enable logging for
const LOGGING_MODULES: [&str; 2] = ["pocket_relay", "actix_web"];

/// Setup function for setting up the Log4rs logging configuring it
/// for all the different modules and and setting up file and stdout logging
pub fn setup() {
    // Configuration from environment
    let logging_level = env::from_env(env::LOGGING_LEVEL);
    let logging_path = env::env(env::LOGGING_DIR);
    let compression = env::from_env(env::LOG_COMPRESSION);

    let pattern = Box::new(PatternEncoder::new(LOGGING_PATTERN));
    let size_trigger = SizeTrigger::new(LOGGING_MAX_SIZE);

    let mut file_pattern = format!("{}/log-{{}}.log", &logging_path);
    // If compression is enable the file uses the .gz extension
    if compression {
        file_pattern.push_str(".gz")
    }

    let latest_path = format!("{}/log.log", &logging_path);

    let fixed_window_roller = FixedWindowRoller::builder()
        .build(&file_pattern, LOGGING_MAX_FILES)
        .expect("Unable to create fixed window log roller");

    let compound_policy =
        CompoundPolicy::new(Box::new(size_trigger), Box::new(fixed_window_roller));

    let stdout_appender = ConsoleAppender::builder().encoder(pattern.clone()).build();

    let file_appender = RollingFileAppender::builder()
        .encoder(pattern)
        .build(&latest_path, Box::new(compound_policy))
        .expect("Unable to create logging file appender");

    const APPENDERS: [&str; 2] = ["stdout", "file"];

    let mut builder = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout_appender)))
        .appender(Appender::builder().build("file", Box::new(file_appender)));

    for module in LOGGING_MODULES {
        builder = builder.logger(
            Logger::builder()
                .appenders(APPENDERS)
                .additive(false)
                .build(module, logging_level),
        )
    }

    let config = builder
        .build(
            Root::builder()
                .appenders(APPENDERS)
                .build(LevelFilter::Warn),
        )
        .expect("Failed to create logging config");

    init_config(config).expect("Unable to initialize logger");
}
