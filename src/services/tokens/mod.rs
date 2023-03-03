//! Authentication token provider and verification service. Makes use of
//! HS256 signed tokens which are 12 bytes 4 for the player ID and 8 for
//! the expiry date

use crate::utils::random::random_string;
use base64ct::{Base64UrlUnpadded, Encoding};
use log::error;
use ring::hmac::{self, HMAC_SHA256};
use std::{
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tokio::fs::{read_to_string, write};

/// Token provider and verification service
pub struct Tokens {
    /// HMAC key used for computing signatures
    key: hmac::Key,
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("Expired token")]
    Expired,
    #[error("Invalid token")]
    Invalid,
}

impl From<base64ct::Error> for VerifyError {
    fn from(_: base64ct::Error) -> Self {
        Self::Invalid
    }
}

impl Tokens {
    /// Expiry time for tokens
    const EXPIRY_TIME: Duration = Duration::from_secs(60 * 60 * 24 * 30 /* 30 Days */);

    pub async fn new() -> Self {
        // Path to the file containing the server secret value
        let secret_path = Path::new("data/secret.bin");

        // Attempt to load existing secret
        let secret: Option<String> = if secret_path.exists() {
            match read_to_string(secret_path).await {
                Ok(value) => Some(value),
                Err(err) => {
                    error!("Failed to read secrets file: {:?}", err);
                    None
                }
            }
        } else {
            None
        };

        let secret = match secret {
            Some(value) => value,
            // Compute and save new secret
            None => Self::create_secret(secret_path).await,
        };

        let key = hmac::Key::new(HMAC_SHA256, secret.as_bytes());
        Self { key }
    }

    /// Creates a new random 64 character secret and attempts to
    /// save it in the secret.bin file
    ///
    /// `path` The path to the secret.bin file
    async fn create_secret(path: &Path) -> String {
        let value = random_string(64);
        if let Err(err) = write(path, &value).await {
            error!("Failed to write secret token to secret.bin: {:?}", err);
        }
        value
    }

    /// Creates a new claim using the provided claim value
    ///
    /// `claim` The token claim value
    /// `id`    The ID of the player to claim for
    pub fn claim(&self, id: u32) -> String {
        // Compute expiry timestamp
        let exp = SystemTime::now()
            .checked_add(Self::EXPIRY_TIME)
            .expect("Expiry timestamp too far into the future")
            .duration_since(UNIX_EPOCH)
            .expect("Clock went backwards")
            .as_secs();

        // Create encoded token value
        let mut data = [0u8; 12];
        data[..4].copy_from_slice(&id.to_be_bytes());
        data[4..].copy_from_slice(&exp.to_be_bytes());
        let data = &data;

        // Encode the message
        let msg = Base64UrlUnpadded::encode_string(data);

        // Create a signature from the raw message bytes
        let sig = hmac::sign(&self.key, data);
        let sig = Base64UrlUnpadded::encode_string(sig.as_ref());

        // Join the message and signature to create the token
        [msg, sig].join(".")
    }

    /// Verifies a token claims returning the claimed ID
    ///
    /// `token` The token to verify
    pub fn verify(&self, token: &str) -> Result<u32, VerifyError> {
        // Split the token parts
        let (msg_raw, sig) = match token.split_once('.') {
            Some(value) => value,
            None => return Err(VerifyError::Invalid),
        };

        // Decode the 12 byte token message
        let mut msg = [0u8; 12];
        Base64UrlUnpadded::decode(msg_raw, &mut msg)?;

        // Decode the message signature
        let sig: Vec<u8> = Base64UrlUnpadded::decode_vec(sig)?;

        // Verify the signature
        if hmac::verify(&self.key, &msg, &sig).is_err() {
            return Err(VerifyError::Invalid);
        }

        // Extract ID and expiration from the msg bytes
        let mut id = [0u8; 4];
        id.copy_from_slice(&msg[..4]);
        let id = u32::from_be_bytes(id);

        let mut exp = [0u8; 8];
        exp.copy_from_slice(&msg[4..]);
        let exp = u64::from_be_bytes(exp);

        // Ensure the timestamp is not expired
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Clock went backwards")
            .as_secs();

        if exp < now {
            return Err(VerifyError::Expired);
        }

        Ok(id)
    }
}
