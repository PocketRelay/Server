//! Service for storing links to all the currenly active
//! authenticated sessions on the server

use crate::utils::hashing::IntHashMap;
use crate::{session::Session, utils::types::PlayerID};
use argon2::password_hash::rand_core::{OsRng, RngCore};
use base64ct::{Base64UrlUnpadded, Encoding};
use interlink::prelude::*;
use log::error;
use ring::hmac::{self, Key, HMAC_SHA256};
use std::{
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::{
    fs::{write, File},
    io::{self, AsyncReadExt},
};

/// Service for storing links to authenticated sessions and
/// functionality for authenticating sessions
pub struct Sessions {
    /// Map of the authenticated players to their session links
    sessions: RwLock<IntHashMap<PlayerID, Link<Session>>>,

    /// HMAC key used for computing signatures
    key: Key,
}

impl Sessions {
    /// Starts a new service returning its link
    pub async fn new() -> Self {
        let key = Self::create_key().await;
        Self {
            sessions: Default::default(),
            key,
        }
    }

    pub fn create_token(&self, player_id: PlayerID) -> String {
        // Compute expiry timestamp
        let exp = SystemTime::now()
            .checked_add(Self::EXPIRY_TIME)
            .expect("Expiry timestamp too far into the future")
            .duration_since(UNIX_EPOCH)
            .expect("Clock went backwards")
            .as_secs();

        // Create encoded token value
        let mut data = [0u8; 12];
        data[..4].copy_from_slice(&player_id.to_be_bytes());
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

    /// Expiry time for tokens
    const EXPIRY_TIME: Duration = Duration::from_secs(60 * 60 * 24 * 30 /* 30 Days */);

    /// Creates a new instance of the tokens structure loading/creating
    /// the secret bytes that are used for signing authentication tokens
    pub async fn create_key() -> Key {
        // Path to the file containing the server secret value
        let secret_path = Path::new("data/secret.bin");

        // The bytes of the secret
        let mut secret = [0u8; 64];

        // Attempt to load existing secret
        if secret_path.exists() {
            if let Err(err) = Self::read_secret(&mut secret, secret_path).await {
                error!("Failed to read secrets file: {:?}", err);
            } else {
                return Key::new(HMAC_SHA256, &secret);
            }
        }

        // Generate random secret bytes
        OsRng.fill_bytes(&mut secret);

        // Save the created secret
        if let Err(err) = write(secret_path, &secret).await {
            error!("Failed to write secrets file: {:?}", err);
        }

        Key::new(HMAC_SHA256, &secret)
    }

    /// Reads the secret from the secrets file into the provided buffer
    /// returning whether the entire secret could be read
    ///
    /// `out` The buffer to read the secret to
    async fn read_secret(out: &mut [u8], path: &Path) -> io::Result<()> {
        let mut file = File::open(path).await?;
        file.read_exact(out).await?;
        Ok(())
    }

    pub fn verify_token(&self, token: &str) -> Result<u32, VerifyError> {
        // Split the token parts
        let (msg_raw, sig_raw) = match token.split_once('.') {
            Some(value) => value,
            None => return Err(VerifyError::Invalid),
        };

        // Decode the 12 byte token message
        let mut msg = [0u8; 12];
        Base64UrlUnpadded::decode(msg_raw, &mut msg)?;

        // Decode 32byte signature (SHA256)
        let mut sig = [0u8; 32];
        Base64UrlUnpadded::decode(sig_raw, &mut sig)?;

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

    pub async fn remove_session(&self, player_id: PlayerID) {
        let sessions = &mut *self.sessions.write().await;
        sessions.remove(&player_id);
    }

    pub async fn add_session(&self, player_id: PlayerID, link: Link<Session>) {
        let sessions = &mut *self.sessions.write().await;
        sessions.insert(player_id, link);
    }

    pub async fn lookup_session(&self, player_id: PlayerID) -> Option<Link<Session>> {
        let sessions = &*self.sessions.read().await;
        sessions.get(&player_id).cloned()
    }
}

/// Errors that can occur while verifying a token
#[derive(Debug, Error)]
pub enum VerifyError {
    /// The token is expired
    #[error("Expired token")]
    Expired,
    /// The token is invalid
    #[error("Invalid token")]
    Invalid,
}

impl From<base64ct::Error> for VerifyError {
    fn from(_: base64ct::Error) -> Self {
        Self::Invalid
    }
}
