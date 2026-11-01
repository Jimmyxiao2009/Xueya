use super::keyring_store::SecretStore;
use crate::error::AppError;
use async_trait::async_trait;
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM, NONCE_LEN};
use ring::rand::{SecureRandom, SystemRandom};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::RwLock;

/// Mobile-safe AES-256-GCM encrypted secret store backed by application-private storage.
///
/// On Android, this protects secrets (API keys, refresh tokens) with AEAD authenticated
/// encryption located inside the app's sandboxed data directory, avoiding plaintext fallback.
pub struct EncryptedSecretStore {
    vault_path: PathBuf,
    key_bytes: [u8; 32],
    cache: RwLock<HashMap<String, String>>,
    rng: SystemRandom,
}

impl EncryptedSecretStore {
    pub fn new(app_data_dir: impl Into<PathBuf>) -> Result<Self, AppError> {
        let app_data_dir = app_data_dir.into();
        let secure_dir = app_data_dir.join("secure");
        std::fs::create_dir_all(&secure_dir)?;

        let key_path = secure_dir.join("vault.key");
        let vault_path = secure_dir.join("vault.enc");

        let rng = SystemRandom::new();
        let mut key_bytes = [0u8; 32];

        match std::fs::read(&key_path) {
            Ok(data) => {
                if data.len() != 32 {
                    return Err(AppError::Internal("vault key has invalid length".into()));
                }
                key_bytes.copy_from_slice(&data);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if vault_path.try_exists()? {
                    return Err(AppError::Internal("existing vault key is missing".into()));
                }
                rng.fill(&mut key_bytes).map_err(|_| {
                    AppError::Internal("failed to generate random vault key".into())
                })?;
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&key_path)?;
                file.write_all(&key_bytes)?;
                file.sync_all()?;
            }
            Err(error) => return Err(error.into()),
        }

        let initial_cache = match std::fs::read(&vault_path) {
            Ok(data) => {
                if data.len() < NONCE_LEN + AES_256_GCM.tag_len() {
                    return Err(AppError::Internal("vault is truncated".into()));
                }
                let (nonce_slice, ciphertext) = data.split_at(NONCE_LEN);
                let unbound = UnboundKey::new(&AES_256_GCM, &key_bytes)
                    .map_err(|_| AppError::Internal("invalid vault key".into()))?;
                let nonce = Nonce::try_assume_unique_for_key(nonce_slice)
                    .map_err(|_| AppError::Internal("invalid vault nonce".into()))?;
                let mut in_out = ciphertext.to_vec();
                let decrypted = LessSafeKey::new(unbound)
                    .open_in_place(nonce, Aad::empty(), &mut in_out)
                    .map_err(|_| {
                        AppError::Internal(
                            "vault authentication failed; original file preserved".into(),
                        )
                    })?;
                serde_json::from_slice::<HashMap<String, String>>(decrypted)
                    .map_err(|_| AppError::Internal("vault contents are invalid".into()))?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(error) => return Err(error.into()),
        };

        Ok(Self {
            vault_path,
            key_bytes,
            cache: RwLock::new(initial_cache),
            rng,
        })
    }

    fn persist(&self, cache: &HashMap<String, String>) -> Result<(), AppError> {
        let json_bytes = serde_json::to_vec(cache)
            .map_err(|e| AppError::Internal(format!("failed to serialize vault: {e}")))?;

        let mut nonce_bytes = [0u8; NONCE_LEN];
        self.rng
            .fill(&mut nonce_bytes)
            .map_err(|_| AppError::Internal("failed to generate nonce".into()))?;

        let unbound = UnboundKey::new(&AES_256_GCM, &self.key_bytes)
            .map_err(|_| AppError::Internal("invalid vault key".into()))?;
        let safe_key = LessSafeKey::new(unbound);
        let nonce = Nonce::try_assume_unique_for_key(&nonce_bytes)
            .map_err(|_| AppError::Internal("invalid nonce".into()))?;

        let mut in_out = json_bytes;
        safe_key
            .seal_in_place_append_tag(nonce, Aad::empty(), &mut in_out)
            .map_err(|_| AppError::Internal("failed to encrypt vault".into()))?;

        let mut output = Vec::with_capacity(NONCE_LEN + in_out.len());
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&in_out);

        let temp_path = self.vault_path.with_extension("tmp");
        let mut file = std::fs::File::create(&temp_path)?;
        file.write_all(&output)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(temp_path, &self.vault_path)
            .map_err(|e| AppError::Internal(format!("failed to save vault: {e}")))?;

        Ok(())
    }
}

#[async_trait]
impl SecretStore for EncryptedSecretStore {
    async fn set_secret(&self, key: &str, value: &str) -> Result<(), AppError> {
        if value.trim().is_empty() {
            return Err(AppError::InvalidInput("secret cannot be empty".into()));
        }
        let mut cache = self
            .cache
            .write()
            .map_err(|_| AppError::Internal("cache lock poisoned".into()))?;
        let mut candidate = cache.clone();
        candidate.insert(key.to_owned(), value.to_owned());
        self.persist(&candidate)?;
        *cache = candidate;
        Ok(())
    }

    async fn get_secret(&self, key: &str) -> Result<String, AppError> {
        let cache = self
            .cache
            .read()
            .map_err(|_| AppError::Internal("cache lock poisoned".into()))?;
        cache
            .get(key)
            .cloned()
            .ok_or_else(|| AppError::NotFound("secret reference does not exist".into()))
    }

    async fn delete_secret(&self, key: &str) -> Result<(), AppError> {
        let mut cache = self
            .cache
            .write()
            .map_err(|_| AppError::Internal("cache lock poisoned".into()))?;
        let mut candidate = cache.clone();
        candidate.remove(key);
        self.persist(&candidate)?;
        *cache = candidate;
        Ok(())
    }

    async fn secret_exists(&self, key: &str) -> Result<bool, AppError> {
        let cache = self
            .cache
            .read()
            .map_err(|_| AppError::Internal("cache lock poisoned".into()))?;
        Ok(cache.contains_key(key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_encrypted_secret_store_roundtrip_and_persistence() {
        let tmp = tempfile::tempdir().unwrap();
        let store = EncryptedSecretStore::new(tmp.path()).unwrap();

        assert!(!store.secret_exists("openai_key").await.unwrap());
        store
            .set_secret("openai_key", "sk-test-12345")
            .await
            .unwrap();
        assert!(store.secret_exists("openai_key").await.unwrap());
        assert_eq!(
            store.get_secret("openai_key").await.unwrap(),
            "sk-test-12345"
        );

        // Verify reopening from disk retains the secret
        let reopened = EncryptedSecretStore::new(tmp.path()).unwrap();
        assert!(reopened.secret_exists("openai_key").await.unwrap());
        assert_eq!(
            reopened.get_secret("openai_key").await.unwrap(),
            "sk-test-12345"
        );

        // Verify deletion
        reopened.delete_secret("openai_key").await.unwrap();
        assert!(!reopened.secret_exists("openai_key").await.unwrap());

        // Reopen again and check absence
        let reopened2 = EncryptedSecretStore::new(tmp.path()).unwrap();
        assert!(!reopened2.secret_exists("openai_key").await.unwrap());
    }
}
