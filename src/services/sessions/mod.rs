//! Service for storing links to all the currenly active
//! authenticated sessions on the server

use crate::{session::Session, utils::types::PlayerID};
use argon2::password_hash::rand_core::{OsRng, RngCore};
use base64ct::{Base64UrlUnpadded, Encoding};
use interlink::prelude::*;
use interlink::service::ServiceContext;
use log::error;
use ring::hmac::{self, Key, HMAC_SHA256};
use std::collections::HashMap;
use std::{
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tokio::{
    fs::{write, File},
    io::{self, AsyncReadExt},
};

/// Service for storing links to authenticated sessions and
/// functionality for authenticating sessions
#[derive(Service)]
pub struct Sessions {
    /// Map of the authenticated players to their session links
    values: HashMap<PlayerID, Link<Session>>,

    /// HMAC key used for computing signatures
    key: Key,
}

/// Message for creating a new authentication token for the provided
/// [PlayerID]
#[derive(Message)]
#[msg(rtype = "String")]
pub struct CreateTokenMessage(pub PlayerID);

/// Message for verifying the provided token
#[derive(Message)]
#[msg(rtype = "Result<PlayerID, VerifyError>")]
pub struct VerifyTokenMessage(pub String);

impl Handler<CreateTokenMessage> for Sessions {
    type Response = Mr<CreateTokenMessage>;

    fn handle(
        &mut self,
        msg: CreateTokenMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        let id = msg.0;

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
        let token = [msg, sig].join(".");

        Mr(token)
    }
}

impl Handler<VerifyTokenMessage> for Sessions {
    type Response = Mr<VerifyTokenMessage>;

    fn handle(
        &mut self,
        msg: VerifyTokenMessage,
        _ctx: &mut ServiceContext<Self>,
    ) -> Self::Response {
        Mr(self.verify(&msg.0))
    }
}

impl Sessions {
    /// Starts a new service returning its link
    pub async fn start() -> Link<Self> {
        let key = Self::create_key().await;
        let this = Self {
            values: Default::default(),
            key,
        };
        this.start()
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

    fn verify(&self, token: &str) -> Result<u32, VerifyError> {
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
}

/// Message for removing players from the authenticated
/// sessions list
#[derive(Message)]
pub struct RemoveMessage {
    /// The ID of the player to remove
    pub player_id: PlayerID,
}

/// Message for adding a player to the authenticated
/// sessions list
#[derive(Message)]
pub struct AddMessage {
    /// The ID of the player the link belongs to
    pub player_id: PlayerID,
    /// The link to the player session
    pub link: Link<Session>,
}

/// Message for finding a session by a player ID returning
/// a link to the player if one is found
#[derive(Message)]
#[msg(rtype = "Option<Link<Session>>")]
pub struct LookupMessage {
    /// The ID of the player to lookup
    pub player_id: PlayerID,
}

/// Handle messages to add authenticated sessions
impl Handler<AddMessage> for Sessions {
    type Response = ();

    fn handle(&mut self, msg: AddMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        self.values.insert(msg.player_id, msg.link);
    }
}

/// Handle messages to remove authenticated sessions
impl Handler<RemoveMessage> for Sessions {
    type Response = ();

    fn handle(&mut self, msg: RemoveMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        self.values.remove(&msg.player_id);
    }
}

/// Handle messages to lookup authenticated sessions
impl Handler<LookupMessage> for Sessions {
    type Response = Mr<LookupMessage>;

    fn handle(&mut self, msg: LookupMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        let value = self.values.get(&msg.player_id).cloned();
        Mr(value)
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
