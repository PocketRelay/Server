use argon2::{Argon2, PasswordHash, PasswordHasher};
use argon2::password_hash::SaltString;
use password_hash::PasswordVerifier;
use rand_core::OsRng;

pub fn hash_password(password: &str) -> password_hash::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2  =Argon2::default();
    let password_hash = argon2.hash_password(password.as_bytes(), &salt)?;
    let value = format!("{}", password_hash);
    Ok(value)
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let hash = match PasswordHash::new(hash) {
        Ok(value) => value,
        _ => return false,
    };
    let argon2  =Argon2::default();
    argon2.verify_password(password.as_bytes(), &hash).is_ok()
}