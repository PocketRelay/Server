use rand::{self, thread_rng, Rng};

/// Generates a random alphanumeric token of the provided length
pub fn generate_token(len: usize) -> String {
    thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}
