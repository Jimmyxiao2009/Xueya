use argon2::password_hash::{
    rand_core::OsRng, Error as PasswordHashError, PasswordHash, PasswordHasher, PasswordVerifier,
    SaltString,
};
use argon2::{Algorithm, Argon2, Params, Version};
use thiserror::Error;

pub const MIN_PASSWORD_LEN: usize = 8;
pub const MAX_PASSWORD_LEN: usize = 256;

#[derive(Debug, Error)]
pub enum PasswordError {
    #[error("password must be between {min} and {max} characters")]
    InvalidLength { min: usize, max: usize },
    #[error("password hashing failed")]
    Hash,
    #[error("invalid credentials")]
    InvalidCredentials,
}

/// Argon2id parameters chosen for a server-side workload. They can be tuned
/// without changing the stored hash format.
pub fn argon2() -> Argon2<'static> {
    let params = Params::new(19 * 1024, 2, 1, None).expect("valid argon2 params");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

pub fn validate_password_shape(password: &str) -> Result<(), PasswordError> {
    let len = password.chars().count();
    if !(MIN_PASSWORD_LEN..=MAX_PASSWORD_LEN).contains(&len) {
        return Err(PasswordError::InvalidLength {
            min: MIN_PASSWORD_LEN,
            max: MAX_PASSWORD_LEN,
        });
    }
    Ok(())
}

pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    validate_password_shape(password)?;
    let salt = SaltString::generate(&mut OsRng);
    argon2()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| PasswordError::Hash)
}

pub fn verify_password(password: &str, password_hash: &str) -> Result<(), PasswordError> {
    let parsed = PasswordHash::new(password_hash).map_err(|_| PasswordError::InvalidCredentials)?;
    argon2()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_: PasswordHashError| PasswordError::InvalidCredentials)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_are_salted_and_verifiable() {
        let first = hash_password("correct horse battery staple").unwrap();
        let second = hash_password("correct horse battery staple").unwrap();
        assert_ne!(first, second);
        assert!(first.starts_with("$argon2id$"));
        verify_password("correct horse battery staple", &first).unwrap();
        assert!(verify_password("wrong password", &first).is_err());
    }

    #[test]
    fn rejects_short_and_long_passwords() {
        assert!(hash_password("short").is_err());
        assert!(hash_password(&"x".repeat(MAX_PASSWORD_LEN + 1)).is_err());
    }

    #[test]
    fn malformed_hash_is_invalid_credentials() {
        assert!(verify_password("whatever", "not-a-hash").is_err());
    }
}
