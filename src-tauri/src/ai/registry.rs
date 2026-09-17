use super::mock::{MockProvider, MOCK_MODEL};
use super::openai_compatible::OpenAiCompatibleProvider;
use super::traits::{ChatProvider, EmbeddingProvider};
use crate::db::repo::providers as provider_repo;
use crate::domain::ProviderConfig;
use crate::error::AppError;
use crate::secrets::keyring_store::SecretStore;
use crate::util::time::utc_now;
use sqlx::SqlitePool;
use std::sync::Arc;
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ProviderUpsert {
    pub id: Option<Uuid>,
    pub display_name: String,
    pub base_url: String,
    pub chat_model: String,
    pub embedding_model: String,
    pub api_key: Option<String>,
    pub enabled: bool,
}

#[derive(Clone)]
pub struct ProviderRegistry {
    pool: SqlitePool,
    http: reqwest::Client,
    secrets: Arc<dyn SecretStore>,
}

impl ProviderRegistry {
    pub fn new(pool: SqlitePool, http: reqwest::Client, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            pool,
            http,
            secrets,
        }
    }

    pub async fn list(&self) -> Result<Vec<ProviderConfig>, AppError> {
        provider_repo::list_providers(&self.pool).await
    }

    pub async fn get(&self, id: Uuid) -> Result<ProviderConfig, AppError> {
        provider_repo::get_provider(&self.pool, id).await
    }

    pub async fn has_api_key(&self, provider: &ProviderConfig) -> Result<bool, AppError> {
        match provider.api_key_ref.as_deref() {
            Some(reference) => self.secrets.secret_exists(reference).await,
            None => Ok(false),
        }
    }

    pub async fn upsert(&self, input: ProviderUpsert) -> Result<ProviderConfig, AppError> {
        validate_provider_input(&input)?;
        let now = utc_now();
        let id = input.id.unwrap_or_else(Uuid::new_v4);
        let existing = if input.id.is_some() {
            provider_repo::get_provider(&self.pool, id).await.ok()
        } else {
            None
        };
        let secret_ref = format!("secret://provider/{id}");
        let api_key_ref = if let Some(api_key) = input
            .api_key
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            self.secrets.set_secret(&secret_ref, api_key.trim()).await?;
            Some(secret_ref)
        } else {
            existing
                .as_ref()
                .and_then(|provider| provider.api_key_ref.clone())
        };

        let provider = ProviderConfig {
            id,
            display_name: input.display_name.trim().to_owned(),
            base_url: input.base_url.trim().trim_end_matches('/').to_owned(),
            chat_model: input.chat_model.trim().to_owned(),
            embedding_model: input.embedding_model.trim().to_owned(),
            api_key_ref,
            enabled: input.enabled,
            created_at: existing
                .as_ref()
                .map(|value| value.created_at)
                .unwrap_or(now),
            updated_at: now,
        };
        provider_repo::upsert_provider(&self.pool, &provider).await?;
        Ok(provider)
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let provider = provider_repo::get_provider(&self.pool, id).await?;
        if let Some(reference) = provider.api_key_ref {
            self.secrets.delete_secret(&reference).await?;
        }
        provider_repo::delete_provider(&self.pool, id).await
    }

    pub async fn chat_provider(
        &self,
        provider: &ProviderConfig,
    ) -> Result<Arc<dyn ChatProvider>, AppError> {
        let key = self.api_key(provider).await?;
        Ok(Arc::new(OpenAiCompatibleProvider::new(
            self.http.clone(),
            provider.base_url.clone(),
            key,
            provider.chat_model.clone(),
            provider.embedding_model.clone(),
        )))
    }

    pub async fn embedding_provider(
        &self,
        provider: &ProviderConfig,
    ) -> Result<Arc<dyn EmbeddingProvider>, AppError> {
        let key = self.api_key(provider).await?;
        Ok(Arc::new(OpenAiCompatibleProvider::new(
            self.http.clone(),
            provider.base_url.clone(),
            key,
            provider.chat_model.clone(),
            provider.embedding_model.clone(),
        )))
    }

    /// The provider used for real work.
    ///
    /// Falls back to [`MockProvider`] when nothing is configured or the stored
    /// key is missing, so that a fresh install is a *working* install. The
    /// caller can tell the two apart with [`is_mock_config`].
    pub async fn default_chat_provider(
        &self,
    ) -> Result<(ProviderConfig, Arc<dyn ChatProvider>), AppError> {
        match provider_repo::first_enabled_provider(&self.pool).await? {
            None => Ok((mock_config(), Arc::new(MockProvider::new()))),
            Some(provider) => match self.chat_provider(&provider).await {
                Ok(chat) => Ok((provider, chat)),
                Err(AppError::ProviderAuth) => {
                    tracing::warn!(
                        provider = %provider.id,
                        "provider has no usable API key; using the offline mock provider"
                    );
                    Ok((mock_config(), Arc::new(MockProvider::new())))
                }
                Err(error) => Err(error),
            },
        }
    }

    pub async fn default_embedding_provider(
        &self,
    ) -> Result<(ProviderConfig, Arc<dyn EmbeddingProvider>), AppError> {
        match provider_repo::first_enabled_provider(&self.pool).await? {
            None => Ok((mock_config(), Arc::new(MockProvider::new()))),
            Some(provider) => match self.embedding_provider(&provider).await {
                Ok(embedding) => Ok((provider, embedding)),
                Err(AppError::ProviderAuth) => Ok((mock_config(), Arc::new(MockProvider::new()))),
                Err(error) => Err(error),
            },
        }
    }

    /// True when the returned config is the offline stand-in.
    pub async fn is_mock_active(&self) -> Result<bool, AppError> {
        let provider = provider_repo::first_enabled_provider(&self.pool).await?;
        match provider {
            None => Ok(true),
            Some(provider) => Ok(self.api_key(&provider).await.is_err()),
        }
    }

    async fn api_key(&self, provider: &ProviderConfig) -> Result<String, AppError> {
        let reference = provider
            .api_key_ref
            .as_deref()
            .ok_or(AppError::ProviderAuth)?;
        self.secrets
            .get_secret(reference)
            .await
            .map_err(|error| match error {
                AppError::NotFound(_) => AppError::ProviderAuth,
                other => other,
            })
    }
}

fn validate_provider_input(input: &ProviderUpsert) -> Result<(), AppError> {
    if input.display_name.trim().is_empty() {
        return Err(AppError::InvalidInput(
            "provider display name is required".into(),
        ));
    }
    let url = Url::parse(input.base_url.trim())
        .map_err(|error| AppError::InvalidInput(format!("invalid provider base URL: {error}")))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::InvalidInput(
            "provider base URL must use http or https".into(),
        ));
    }
    if input.chat_model.trim().is_empty() {
        return Err(AppError::InvalidInput("chat model is required".into()));
    }
    if input.embedding_model.trim().is_empty() {
        return Err(AppError::InvalidInput("embedding model is required".into()));
    }
    Ok(())
}

/// A synthetic provider record for the offline stand-in. Nothing is ever sent
/// to `baseUrl`; the scheme makes that obvious in logs and in the UI.
pub fn mock_config() -> ProviderConfig {
    let now = utc_now();
    ProviderConfig {
        id: Uuid::nil(),
        display_name: "Offline Mock".to_owned(),
        base_url: "mock://local".to_owned(),
        chat_model: MOCK_MODEL.to_owned(),
        embedding_model: MOCK_MODEL.to_owned(),
        api_key_ref: None,
        enabled: true,
        created_at: now,
        updated_at: now,
    }
}

pub fn is_mock_config(provider: &ProviderConfig) -> bool {
    provider.base_url.starts_with("mock://")
}
