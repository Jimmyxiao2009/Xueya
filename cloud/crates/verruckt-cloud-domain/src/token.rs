use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum TokenError {
    #[error("invalid access token")]
    InvalidAccessToken,
    #[error("access token has expired")]
    ExpiredAccessToken,
    #[error("access token signing failed")]
    Signing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessTokenClaims {
    /// User id.
    pub sub: Uuid,
    /// Session id.
    pub sid: Uuid,
    /// Issued at (unix seconds).
    pub iat: i64,
    /// Expires at (unix seconds).
    pub exp: i64,
    /// Issuer.
    pub iss: String,
}

impl AccessTokenClaims {
    pub fn new(user_id: Uuid, session_id: Uuid, ttl: Duration, issuer: &str) -> Self {
        let now = Utc::now();
        Self {
            sub: user_id,
            sid: session_id,
            iat: now.timestamp(),
            exp: (now + ttl).timestamp(),
            iss: issuer.to_string(),
        }
    }
}

pub fn encode_access_token(
    claims: &AccessTokenClaims,
    secret: &[u8],
) -> Result<String, TokenError> {
    encode(
        &Header::new(Algorithm::HS256),
        claims,
        &EncodingKey::from_secret(secret),
    )
    .map_err(|_| TokenError::Signing)
}

pub fn decode_access_token(
    token: &str,
    secret: &[u8],
    issuer: &str,
) -> Result<AccessTokenClaims, TokenError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&[issuer]);
    validation.validate_exp = true;
    validation.leeway = 5;

    decode::<AccessTokenClaims>(token, &DecodingKey::from_secret(secret), &validation)
        .map(|data| data.claims)
        .map_err(|error| match error.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => TokenError::ExpiredAccessToken,
            _ => TokenError::InvalidAccessToken,
        })
}

/// Returns a high-entropy refresh token. The token is URL-safe so clients can
/// store it without extra encoding.
pub fn generate_refresh_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Refresh tokens are stored only as SHA-256 hex digests.
pub fn hash_refresh_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    hex::encode(digest)
}

pub fn session_expiry(now: DateTime<Utc>, days: i64) -> DateTime<Utc> {
    now + Duration::days(days)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_token_round_trip() {
        let user_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        let claims = AccessTokenClaims::new(
            user_id,
            session_id,
            Duration::minutes(15),
            "verruckt-cloud-test",
        );
        let token = encode_access_token(&claims, b"secret").unwrap();
        let decoded = decode_access_token(&token, b"secret", "verruckt-cloud-test").unwrap();
        assert_eq!(decoded.sub, user_id);
        assert_eq!(decoded.sid, session_id);
    }

    #[test]
    fn wrong_secret_is_rejected() {
        let claims = AccessTokenClaims::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Duration::minutes(15),
            "verruckt-cloud-test",
        );
        let token = encode_access_token(&claims, b"secret").unwrap();
        assert!(decode_access_token(&token, b"other", "verruckt-cloud-test").is_err());
    }

    #[test]
    fn refresh_tokens_are_random_and_hashed_stably() {
        let first = generate_refresh_token();
        let second = generate_refresh_token();
        assert_ne!(first, second);
        assert_eq!(hash_refresh_token(&first), hash_refresh_token(&first));
        assert_ne!(hash_refresh_token(&first), first);
    }
}
