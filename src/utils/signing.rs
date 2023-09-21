use argon2::password_hash::rand_core::{OsRng, RngCore};
use log::{debug, error};
use ring::hmac::{self, Key, Tag, HMAC_SHA256};
use std::{io, path::Path};
use tokio::{
    fs::{write, File},
    io::AsyncReadExt,
};

pub struct SigningKey(Key);

impl AsRef<Key> for SigningKey {
    fn as_ref(&self) -> &Key {
        &self.0
    }
}

impl SigningKey {
    const KEY_LENGTH: usize = 64;

    /// Obtains the global signing key by reading it from a file
    /// or generating a new one and saving that to a file
    ///
    /// Should only be used by the actual app, tests should
    /// generate a new signing key
    pub async fn global() -> Self {
        // Path to the file containing the server secret value
        let secret_path = Path::new("data/secret.bin");

        if secret_path.exists() {
            match Self::from_file(secret_path).await {
                Ok(value) => return value,
                Err(err) => {
                    error!("Failed to load existing secrets file: {}", err);
                }
            }
        }

        debug!("Generating server secret key...");
        let (key, secret) = Self::generate();
        if let Err(err) = write(secret_path, &secret).await {
            error!("Failed to save secrets file: {}", err);
        }

        key
    }

    #[inline]
    fn new(secret: &[u8; Self::KEY_LENGTH]) -> Self {
        Self(Key::new(HMAC_SHA256, secret))
    }

    #[inline]
    pub fn sign(&self, data: &[u8]) -> Tag {
        hmac::sign(&self.0, data)
    }

    #[inline]
    pub fn verify(&self, data: &[u8], tag: &[u8]) -> bool {
        hmac::verify(&self.0, data, tag).is_ok()
    }

    /// Generates a new signing key
    pub fn generate() -> (Self, [u8; Self::KEY_LENGTH]) {
        let mut secret = [0; Self::KEY_LENGTH];
        OsRng.fill_bytes(&mut secret);
        (Self::new(&secret), secret)
    }

    // Attempts to read a signing key from the provided file
    async fn from_file(file: &Path) -> io::Result<SigningKey> {
        let mut secret = [0; Self::KEY_LENGTH];
        let mut file = File::open(file).await?;
        file.read_exact(&mut secret).await?;
        Ok(Self::new(&secret))
    }
}
