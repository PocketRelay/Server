//! Service for storing links to all the currenly active
//! authenticated sessions on the server

use crate::utils::hashing::IntHashMap;
use crate::utils::types::PlayerID;
use crate::{session::SessionLink, utils::signing::SigningKey};
use base64ct::{Base64UrlUnpadded, Encoding};
use log::error;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::RwLock;

/// Service for storing links to authenticated sessions and
/// functionality for authenticating sessions
pub struct Sessions {
    /// Map of the authenticated players to their session links
    sessions: RwLock<IntHashMap<PlayerID, SessionLink>>,

    /// HMAC key used for computing signatures
    key: SigningKey,
}

impl Sessions {
    /// Starts a new service returning its link
    pub fn new(key: SigningKey) -> Self {
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
        let sig = self.key.sign(data);
        let sig = Base64UrlUnpadded::encode_string(sig.as_ref());

        // Join the message and signature to create the token
        [msg, sig].join(".")
    }

    /// Expiry time for tokens
    const EXPIRY_TIME: Duration = Duration::from_secs(60 * 60 * 24 * 30 /* 30 Days */);

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
        if !self.key.verify(&msg, &sig) {
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

    pub async fn add_session(&self, player_id: PlayerID, link: SessionLink) {
        let sessions = &mut *self.sessions.write().await;
        sessions.insert(player_id, link);
    }

    pub async fn lookup_session(&self, player_id: PlayerID) -> Option<SessionLink> {
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
