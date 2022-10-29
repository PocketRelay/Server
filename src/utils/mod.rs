use std::time::{Duration, SystemTime, UNIX_EPOCH};
use rand::{Rng, thread_rng};

pub mod hashing;
pub mod dmap;
pub mod conv;

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
    now
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}