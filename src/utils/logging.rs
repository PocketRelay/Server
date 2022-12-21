use crate::{env, utils::net::public_address};
use flexi_logger::{
    DeferredNow, Duplicate, FileSpec, LogSpecification, Logger, Record,
    TS_DASHES_BLANK_COLONS_DOT_BLANK,
};
use log::{info, LevelFilter};

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

    let spec = LogSpecification::builder()
        // Apply the environment logging level to the pocket_relay module
        .module("pocket_relay", logging_level)
        // Apply the warning level to all other modules
        .default(LevelFilter::Warn)
        .build();

    let file_spec = FileSpec::default().directory(logging_path).basename("log");

    let logger = Logger::with(spec)
        .format_for_stdout(log_format)
        .log_to_file(file_spec)
        .duplicate_to_stdout(Duplicate::All);

    if let Err(err) = logger.start() {
        eprintln!("Failed to start logger: {err:?}")
    }
}

pub fn log_format(
    w: &mut dyn std::io::Write,
    now: &mut DeferredNow,
    record: &Record,
) -> Result<(), std::io::Error> {
    write!(
        w,
        "[{}] [{}] [{}] {}",
        now.format(TS_DASHES_BLANK_COLONS_DOT_BLANK),
        record.level(),
        record.module_path().unwrap_or("<unnamed>"),
        record.args()
    )
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

        output.push_str("WAN: ");
        output.push_str(&public_address);
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
