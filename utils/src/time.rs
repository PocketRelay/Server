use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Returns the current server unix timestamp in seconds.
pub fn server_unix_time() -> u64 {
    let now = SystemTime::now();
    now.duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}
