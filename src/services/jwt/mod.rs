use std::path::Path;

use crate::utils::random::random_string;
use chrono::{Days, Utc};

use hs256_token::{JsonError, Tokens};
use log::error;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::fs::{read_to_string, write};
/// Json Web Token service for providing JWT tokens
/// and token claiming
pub struct Jwt {
    tokens: Tokens,
}

impl Jwt {
    pub async fn new() -> Self {
        // Path to the file containing the server secret value
        let token_path = Path::new("data/secret.bin");

        let mut secret: Option<String> = None;

        if token_path.exists() {
            secret = match read_to_string(token_path).await {
                Ok(value) => Some(value),
                Err(err) => {
                    error!("Failed to read secrets file: {:?}", err);
                    None
                }
            };
        }

        let secret = match secret {
            Some(value) => value,
            None => {
                let value = random_string(64);
                if let Err(err) = write(token_path, &value).await {
                    error!("Failed to write secret token to secret.bin: {:?}", err);
                }
                value
            }
        };

        let secret_bytes = secret.as_bytes();
        let tokens = Tokens::new(secret_bytes);
        Self { tokens }
    }

    /// Creates a new claim using the provided claim value
    ///
    /// `claim` The token claim value
    /// `id`    The ID of the player to claim for
    pub fn claim(&self, id: u32) -> Result<String, ClaimError> {
        let exp = Utc::now()
            .checked_add_days(Days::new(30))
            .ok_or(ClaimError::Timestamp)?
            .timestamp();
        let claim = PlayerClaim { id, exp };
        let token = self.tokens.encode(&claim)?;
        Ok(token)
    }

    /// Verifies a token claims returning the decoded claim structure
    ///
    /// `token` The token to verify
    pub fn verify(&self, token: &str) -> Result<PlayerClaim, VerifyError> {
        let claim: PlayerClaim = self
            .tokens
            .decode(token)
            .map_err(|_| VerifyError::Invalid)?;
        let now = Utc::now().timestamp();
        if claim.exp < now {
            return Err(VerifyError::Expired);
        }
        Ok(claim)
    }
}

#[derive(Debug, Error)]
pub enum ClaimError {
    #[error("{0}")]
    Json(#[from] JsonError),
    #[error("Failed to create timestamp for message")]
    Timestamp,
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("Expired token")]
    Expired,
    #[error("Invalid token")]
    Invalid,
}

/// Claim for player authentication
#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerClaim {
    /// The ID of the user this claim represents
    pub id: u32,
    /// Expiry date timestamp
    pub exp: i64,
}
