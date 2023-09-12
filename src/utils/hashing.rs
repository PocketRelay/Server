//! Hashing utility for hashing and verifying passwords

use std::{
    collections::HashMap,
    hash::{BuildHasherDefault, Hasher},
};

use argon2::{
    password_hash::{self, rand_core::OsRng, PasswordVerifier, SaltString},
    Argon2, PasswordHash, PasswordHasher,
};

/// Hashes the provided password using the Argon2 algorithm returning
/// the generated hash in string form.
///
/// `password` The password to hash
pub fn hash_password(password: &str) -> password_hash::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(password.as_bytes(), &salt)?;
    let value = format!("{}", password_hash);
    Ok(value)
}

/// Verifies the hash of the provided password checking that
/// it matches the provided hash
///
/// `password` The plain text password
/// `hash`     The hashed password
pub fn verify_password(password: &str, hash: &str) -> bool {
    let hash = match PasswordHash::new(hash) {
        Ok(value) => value,
        _ => return false,
    };
    let argon2 = Argon2::default();
    argon2.verify_password(password.as_bytes(), &hash).is_ok()
}

pub type IntHashMap<K, V> = HashMap<K, V, BuildHasherDefault<IntHasher>>;

/// Hasher implementation that directly uses an integer value
/// instead of any specific hashing algorithm
///
/// Only implements hashing for [u32] and [u64]
///
/// Used for hashing packet component paths and type Ids
#[derive(Default)]
pub struct IntHasher(u64);

impl Hasher for IntHasher {
    fn write(&mut self, _: &[u8]) {
        panic!("Attempted to use int hasher to hash bytes")
    }

    #[inline]
    fn write_u64(&mut self, id: u64) {
        self.0 = id;
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.0 = i as u64;
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}
