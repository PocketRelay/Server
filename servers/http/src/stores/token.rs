use core::env;
use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};

use tokio::sync::Mutex;
use utils::random::generate_random_string;

/// The life duration of a session token (How long the session tokens will be valid for)
/// currently set to 1day worth of life
const TOKEN_LIFE_DURATION: Duration = Duration::from_millis(60 * 60 * 24);
/// The length of randomly generated token to create
const TOKEN_LENGTH: usize = 128;

pub struct TokenStore {
    /// Hash map of tokens mapped to the time that they will
    /// become expired at.
    tokens: Mutex<HashMap<String, SystemTime>>,
}

impl TokenStore {
    pub fn new() -> Self {
        Self {
            tokens: Mutex::new(HashMap::new()),
        }
    }

    /// Checks if the provided token is valid. If the token is
    /// expired then it is removed from the token store
    ///
    /// `token` The token to check the validity of
    pub async fn is_valid_token(&self, token: &str) -> bool {
        let tokens = &mut *self.tokens.lock().await;
        let now = SystemTime::now();
        tokens.retain(|_, value| now.lt(value));
        tokens.contains_key(token)
    }

    /// Attempts to authenticate a session with the provided username and password.
    /// Checks the API environment variables and will return a generated token
    /// if it was a success or None if the credentials were incorrect
    ///
    /// `username` The username to authenticate with
    /// `password` The password to authenticate with
    pub async fn authenticate(&self, username: &str, password: &str) -> Option<String> {
        let api_username = env::env(env::API_USERNAME);
        let api_password = env::env(env::API_PASSWORD);

        if api_username.ne(username) || api_password.ne(password) {
            return None;
        }

        let tokens = &mut *self.tokens.lock().await;
        let mut token: String;
        loop {
            token = generate_random_string(TOKEN_LENGTH);
            if !tokens.contains_key(&token) {
                break;
            }
        }

        let expiry_time = SystemTime::now() + TOKEN_LIFE_DURATION;
        tokens.insert(token.clone(), expiry_time);
        Some(token)
    }
}
