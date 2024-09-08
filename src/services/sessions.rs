//! Service for storing links to all the currenly active
//! authenticated sessions on the server

use crate::session::{SessionLink, WeakSessionLink};
use crate::utils::hashing::IntHashMap;
use crate::utils::signing::SigningKey;
use crate::utils::types::PlayerID;
use base64ct::{Base64UrlUnpadded, Encoding};
use hashbrown::HashMap;
use parking_lot::Mutex;
use rand::distributions::Distribution;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use uuid::Uuid;

type SessionMap = IntHashMap<PlayerID, WeakSessionLink>;

pub type LoginCode = String;

/// Service for storing links to authenticated sessions and
/// functionality for authenticating sessions
pub struct Sessions {
    /// Lookup mapping between player IDs and their session links
    ///
    /// This uses a blocking mutex as there is little to no overhead
    /// since all operations are just map read and writes which don't
    /// warrant the need for the async variant
    sessions: Mutex<SessionMap>,

    /// Mapping between generated login codes and the user the code
    /// will login
    login_codes: Mutex<HashMap<LoginCode, LoginCodeData>>,

    /// HMAC key used for computing signatures
    key: SigningKey,
}

pub struct LoginCodeData {
    /// ID of the player the code is for
    player_id: PlayerID,
    /// Timestamp when the code expires
    exp: SystemTime,
}

/// Unique ID given to clients before connecting so that session
/// connections can be associated with network tunnels without
/// relying on IP addresses: https://github.com/PocketRelay/Server/issues/64#issuecomment-1867015578
pub type AssociationId = Uuid;

/// Rand distribution for a logic code part
struct LoginCodePart;

impl Distribution<char> for LoginCodePart {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> char {
        let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let idx = rng.gen_range(0..chars.len());
        chars[idx] as char
    }
}
impl Sessions {
    /// Expiry time for tokens
    const EXPIRY_TIME: Duration = Duration::from_secs(60 * 60 * 24 * 30 /* 30 Days */);

    /// Expiry time for tokens
    const LOGIN_CODE_EXPIRY_TIME: Duration = Duration::from_secs(60 * 30 /* 30 minutes */);

    /// Starts a new service returning its link
    pub fn new(key: SigningKey) -> Self {
        Self {
            sessions: Default::default(),
            login_codes: Default::default(),
            key,
        }
    }

    /// Creates a new login code for the provider player, returns the
    /// login code storing the data so it can be exchanged
    pub fn create_login_code(&self, player_id: PlayerID) -> Result<LoginCode, ()> {
        let rng = StdRng::from_entropy();

        let code: LoginCode = rng
            .sample_iter(&LoginCodePart)
            .take(5)
            .map(char::from)
            .collect();

        // Compute expiry timestamp
        let exp = SystemTime::now()
            .checked_add(Self::LOGIN_CODE_EXPIRY_TIME)
            .expect("Expiry timestamp too far into the future");

        // Store the code so they can login
        self.login_codes
            .lock()
            .insert(code.clone(), LoginCodeData { player_id, exp });

        Ok(code)
    }

    /// Exchanges a login code for a token to the player the code was for
    /// if the token is not expired
    pub fn exchange_login_code(&self, login_code: &LoginCode) -> Option<(PlayerID, String)> {
        let data = self.login_codes.lock().remove(login_code)?;

        // Login code is expired
        if data.exp.lt(&SystemTime::now()) {
            return None;
        }

        let player_id = data.player_id;

        let token = self.create_token(player_id);
        Some((player_id, token))
    }

    /// Creates a new association token
    pub fn create_assoc_token(&self) -> String {
        let uuid = Uuid::new_v4();
        let data: &[u8; 16] = uuid.as_bytes();
        // Encode the message
        let msg = Base64UrlUnpadded::encode_string(data);

        // Create a signature from the raw message bytes
        let sig = self.key.sign(data);
        let sig = Base64UrlUnpadded::encode_string(sig.as_ref());

        // Join the message and signature to create the token
        [msg, sig].join(".")
    }

    /// Verifies an association token
    pub fn verify_assoc_token(&self, token: &str) -> Result<AssociationId, VerifyError> {
        // Split the token parts
        let (msg_raw, sig_raw) = match token.split_once('.') {
            Some(value) => value,
            None => return Err(VerifyError::Invalid),
        };

        // Decode the 16 byte token message
        let mut msg = [0u8; 16];
        Base64UrlUnpadded::decode(msg_raw, &mut msg).map_err(|_| VerifyError::Invalid)?;

        // Decode 32byte signature (SHA256)
        let mut sig = [0u8; 32];
        Base64UrlUnpadded::decode(sig_raw, &mut sig).map_err(|_| VerifyError::Invalid)?;

        // Verify the signature
        if !self.key.verify(&msg, &sig) {
            return Err(VerifyError::Invalid);
        }
        let uuid = *Uuid::from_bytes_ref(&msg);
        Ok(uuid)
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

    pub fn verify_token(&self, token: &str) -> Result<u32, VerifyError> {
        // Split the token parts
        let (msg_raw, sig_raw) = match token.split_once('.') {
            Some(value) => value,
            None => return Err(VerifyError::Invalid),
        };

        // Decode the 12 byte token message
        let mut msg = [0u8; 12];
        Base64UrlUnpadded::decode(msg_raw, &mut msg).map_err(|_| VerifyError::Invalid)?;

        // Decode 32byte signature (SHA256)
        let mut sig = [0u8; 32];
        Base64UrlUnpadded::decode(sig_raw, &mut sig).map_err(|_| VerifyError::Invalid)?;

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

    pub fn remove_session(&self, player_id: PlayerID) {
        let sessions = &mut *self.sessions.lock();
        sessions.remove(&player_id);
    }

    pub fn add_session(&self, player_id: PlayerID, link: WeakSessionLink) {
        let sessions = &mut *self.sessions.lock();
        sessions.insert(player_id, link);
    }

    pub fn lookup_session(&self, player_id: PlayerID) -> Option<SessionLink> {
        let sessions = &mut *self.sessions.lock();
        let session = sessions.get(&player_id)?;
        let session = match session.upgrade() {
            Some(value) => value,
            // Session has stopped remove it from the map
            None => {
                sessions.remove(&player_id);
                return None;
            }
        };

        Some(session)
    }
}

/// Errors that can occur while verifying a token
#[derive(Debug, Error)]
pub enum VerifyError {
    /// The token is expired
    #[error("token is expired")]
    Expired,
    /// The token is invalid
    #[error("token is invalid")]
    Invalid,
}

#[cfg(test)]
mod test {
    use crate::utils::signing::SigningKey;

    use super::Sessions;

    /// Tests that tokens can be created and verified correctly
    #[test]
    fn test_token() {
        let (key, _) = SigningKey::generate();
        let sessions = Sessions::new(key);

        let player_id = 32;
        let token = sessions.create_token(player_id);
        let claim = sessions.verify_token(&token).unwrap();

        assert_eq!(player_id, claim)
    }
}
