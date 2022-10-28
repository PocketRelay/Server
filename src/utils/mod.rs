use rand::{Rng, thread_rng};

pub mod hashing;
pub mod dmap;
pub mod conv;

pub fn generate_token(len: usize) -> String {
    thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}