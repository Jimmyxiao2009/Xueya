use crate::error::AppError;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

#[async_trait]
pub trait SecretStore: Send + Sync {
    async fn set_secret(&self, key: &str, value: &str) -> Result<(), AppError>;
    async fn get_secret(&self, key: &str) -> Result<String, AppError>;
    async fn delete_secret(&self, key: &str) -> Result<(), AppError>;
    async fn secret_exists(&self, key: &str) -> Result<bool, AppError>;
}

pub struct KeyringSecretStore {
    service: String,
}

impl KeyringSecretStore {
    pub fn new() -> Self {
        Self {
            service: "app.verruckt.desktop".to_owned(),
        }
    }

    fn entry(&self, key: &str) -> Result<keyring::Entry, AppError> {
        keyring::Entry::new(&self.service, key)
            .map_err(|error| AppError::Internal(format!("credential store unavailable: {error}")))
    }
}

impl Default for KeyringSecretStore {
    fn default() -> Self {
        Self::new()
    }
}

fn is_missing(error: &keyring::Error) -> bool {
    let text = error.to_string().to_ascii_lowercase();
    text.contains("no entry") || text.contains("not found") || text.contains("not present")
}

#[async_trait]
impl SecretStore for KeyringSecretStore {
    async fn set_secret(&self, key: &str, value: &str) -> Result<(), AppError> {
        if value.trim().is_empty() {
            return Err(AppError::InvalidInput("secret cannot be empty".into()));
        }
        let entry = self.entry(key)?;
        entry
            .set_password(value)
            .map_err(|error| AppError::Internal(format!("credential store write failed: {error}")))
    }

    async fn get_secret(&self, key: &str) -> Result<String, AppError> {
        let entry = self.entry(key)?;
        match entry.get_password() {
            Ok(value) => Ok(value),
            Err(error) if is_missing(&error) => {
                Err(AppError::NotFound("secret reference does not exist".into()))
            }
            Err(error) => {
                Err(AppError::ProviderAuth
                    .context(format!("credential store read failed: {error}")))
            }
        }
    }

    async fn delete_secret(&self, key: &str) -> Result<(), AppError> {
        let entry = self.entry(key)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(error) if is_missing(&error) => Ok(()),
            Err(error) => Err(AppError::Internal(format!(
                "credential store delete failed: {error}"
            ))),
        }
    }

    async fn secret_exists(&self, key: &str) -> Result<bool, AppError> {
        let entry = self.entry(key)?;
        match entry.get_password() {
            Ok(_) => Ok(true),
            Err(error) if is_missing(&error) => Ok(false),
            Err(error) => Err(AppError::Internal(format!(
                "credential store query failed: {error}"
            ))),
        }
    }
}

/// Used by unit tests and as an explicit opt-in fallback for platforms where
/// no credential service is available. Production desktop paths use the OS
/// keyring implementation above.
#[derive(Default)]
pub struct InMemorySecretStore {
    values: RwLock<HashMap<String, String>>,
}

impl InMemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SecretStore for InMemorySecretStore {
    async fn set_secret(&self, key: &str, value: &str) -> Result<(), AppError> {
        self.values
            .write()
            .map_err(|_| AppError::Internal("secret store poisoned".into()))?
            .insert(key.to_owned(), value.to_owned());
        Ok(())
    }

    async fn get_secret(&self, key: &str) -> Result<String, AppError> {
        self.values
            .read()
            .map_err(|_| AppError::Internal("secret store poisoned".into()))?
            .get(key)
            .cloned()
            .ok_or_else(|| AppError::NotFound("secret reference does not exist".into()))
    }

    async fn delete_secret(&self, key: &str) -> Result<(), AppError> {
        self.values
            .write()
            .map_err(|_| AppError::Internal("secret store poisoned".into()))?
            .remove(key);
        Ok(())
    }

    async fn secret_exists(&self, key: &str) -> Result<bool, AppError> {
        Ok(self
            .values
            .read()
            .map_err(|_| AppError::Internal("secret store poisoned".into()))?
            .contains_key(key))
    }
}
