use crate::env;
use crate::utils::random::generate_random_string;
use axum::Extension;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::Mutex;

/// Structure of a store which stores tokens along with
/// their expiry time used for storing and checking
/// validity of session tokens
#[derive(Default)]
pub struct TokenStore {
    /// Hash map of tokens mapped to the time that they will
    /// become expired at.
    tokens: Mutex<HashMap<String, SystemTime>>,
}

impl TokenStore {
    /// Creates a token store extension to supply to the router
    pub fn extension() -> Extension<Arc<TokenStore>> {
        Extension(Default::default())
    }

    /// The amount of time it takes for a session token to expire.
    /// currently set to 1day before expiring
    const EXPIRY_TIME: Duration = Duration::from_secs(60 * 60 * 24);

    /// The length of randomly generated token to create
    const TOKEN_LENGTH: usize = 64;

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

    /// Removes the provided token from the map of tokens.
    ///
    /// `token` The token to removes
    pub async fn remove_token(&self, token: &str) {
        let tokens = &mut *self.tokens.lock().await;
        tokens.remove(token);
    }

    /// Finds the expiry time for the provided token if it
    /// exists in the tokens map.
    ///
    /// `token` The token to find the expiry time for
    pub async fn get_token_expiry(&self, token: &str) -> Option<SystemTime> {
        let tokens = &*self.tokens.lock().await;
        let expiry_time = tokens.get(token);
        expiry_time.copied()
    }

    /// Attempts to authenticate a session with the provided username and password.
    /// Checks the API environment variables and will return a generated token
    /// if it was a success or None if the credentials were incorrect
    ///
    /// `username` The username to authenticate with
    /// `password` The password to authenticate with
    pub async fn authenticate(
        &self,
        username: &str,
        password: &str,
    ) -> Option<(String, SystemTime)> {
        let api_username = env::env(env::API_USERNAME);
        let api_password = env::env(env::API_PASSWORD);

        if api_username.ne(username) || api_password.ne(password) {
            return None;
        }

        let tokens = &mut *self.tokens.lock().await;
        let mut token: String;
        loop {
            token = generate_random_string(Self::TOKEN_LENGTH);
            if !tokens.contains_key(&token) {
                break;
            }
        }

        let expiry_time = SystemTime::now() + Self::EXPIRY_TIME;
        tokens.insert(token.clone(), expiry_time);
        Some((token, expiry_time))
    }
}
