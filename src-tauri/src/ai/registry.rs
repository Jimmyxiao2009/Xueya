use super::capabilities::ProviderCapabilities;
use super::cloud::CloudAiProvider;
use super::endpoint::normalize_base_url;
use super::mock::{MockProvider, MOCK_MODEL};
use super::openai_compatible::OpenAiCompatibleProvider;
use super::traits::{ChatProvider, EmbeddingProvider};
use crate::cloud::CloudAccountClient;
use crate::db::repo::providers as provider_repo;
use crate::domain::{AiMode, ProviderConfig, ProviderProtocol};
use crate::error::AppError;
use crate::secrets::keyring_store::SecretStore;
use crate::util::time::utc_now;
use sqlx::SqlitePool;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ProviderUpsert {
    pub id: Option<Uuid>,
    pub display_name: String,
    pub base_url: String,
    pub chat_model: String,
    pub embedding_model: String,
    pub api_key: Option<String>,
    pub protocol: ProviderProtocol,
    pub enabled: bool,
}

#[derive(Clone)]
pub struct ProviderRegistry {
    pool: SqlitePool,
    http: reqwest::Client,
    secrets: Arc<dyn SecretStore>,
    cloud: Option<Arc<CloudAccountClient>>,
}

impl ProviderRegistry {
    pub fn new(pool: SqlitePool, http: reqwest::Client, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            pool,
            http,
            secrets,
            cloud: None,
        }
    }

    pub fn with_cloud(mut self, cloud: Arc<CloudAccountClient>) -> Self {
        self.cloud = Some(cloud);
        self
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

    pub async fn api_key_hint(
        &self,
        provider: &ProviderConfig,
    ) -> Result<Option<String>, AppError> {
        let reference = match provider.api_key_ref.as_deref() {
            Some(reference) => reference,
            None => return Ok(None),
        };
        let key = match self.secrets.get_secret(reference).await {
            Ok(key) => key,
            Err(AppError::NotFound(_)) => return Ok(None),
            Err(error) => return Err(error),
        };
        let chars: Vec<char> = key.chars().collect();
        if chars.len() <= 8 {
            return Ok(Some("configured".to_owned()));
        }
        let prefix: String = chars.iter().take(3).collect();
        let suffix: String = chars
            .iter()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        Ok(Some(format!("{prefix}…{suffix}")))
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
            base_url: normalize_base_url(&input.base_url)?,
            chat_model: input.chat_model.trim().to_owned(),
            embedding_model: input.embedding_model.trim().to_owned(),
            api_key_ref,
            protocol: input.protocol,
            enabled: input.enabled,
            created_at: existing
                .as_ref()
                .map(|value| value.created_at)
                .unwrap_or(now),
            updated_at: now,
        };
        provider_repo::upsert_provider(&self.pool, &provider).await?;
        self.clear_capability_cache(id).await?;
        Ok(provider)
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let provider = provider_repo::get_provider(&self.pool, id).await?;
        if let Some(reference) = provider.api_key_ref {
            self.secrets.delete_secret(&reference).await?;
        }
        self.clear_capability_cache(id).await?;
        provider_repo::delete_provider(&self.pool, id).await
    }

    pub async fn chat_provider(
        &self,
        provider: &ProviderConfig,
    ) -> Result<Arc<dyn ChatProvider>, AppError> {
        let key = self.api_key(provider).await?;
        let protocol = self.effective_protocol(provider).await?;
        Ok(Arc::new(OpenAiCompatibleProvider::with_protocol(
            self.http.clone(),
            provider.base_url.clone(),
            key,
            provider.chat_model.clone(),
            provider.embedding_model.clone(),
            protocol,
        )))
    }

    pub async fn embedding_provider(
        &self,
        provider: &ProviderConfig,
    ) -> Result<Arc<dyn EmbeddingProvider>, AppError> {
        let key = self.api_key(provider).await?;
        Ok(Arc::new(OpenAiCompatibleProvider::with_protocol(
            self.http.clone(),
            provider.base_url.clone(),
            key,
            provider.chat_model.clone(),
            provider.embedding_model.clone(),
            ProviderProtocol::ChatCompletions,
        )))
    }

    pub async fn effective_protocol(
        &self,
        provider: &ProviderConfig,
    ) -> Result<ProviderProtocol, AppError> {
        if provider.protocol != ProviderProtocol::Auto {
            return Ok(provider.protocol);
        }
        let capabilities = self.capabilities(provider).await?;
        Ok(capabilities.preferred_protocol())
    }

    pub async fn capabilities(
        &self,
        provider: &ProviderConfig,
    ) -> Result<ProviderCapabilities, AppError> {
        if let Some(cached) = self.load_capability_cache(provider.id).await? {
            return Ok(cached);
        }
        let key = match self.api_key(provider).await {
            Ok(key) => key,
            Err(AppError::ProviderAuth) => {
                return Ok(ProviderCapabilities {
                    probe_errors: vec!["Unauthorized: API key is missing".to_owned()],
                    ..ProviderCapabilities::default()
                });
            }
            Err(error) => return Err(error),
        };
        let client = OpenAiCompatibleProvider::with_protocol(
            self.http.clone(),
            provider.base_url.clone(),
            key,
            provider.chat_model.clone(),
            provider.embedding_model.clone(),
            provider.protocol,
        );
        let capabilities = client.probe_capabilities().await;
        self.save_capability_cache(provider.id, &capabilities)
            .await?;
        Ok(capabilities)
    }

    pub async fn probe_and_cache(&self, id: Uuid) -> Result<ProviderCapabilities, AppError> {
        let provider = provider_repo::get_provider(&self.pool, id).await?;
        self.clear_capability_cache(id).await?;
        self.capabilities(&provider).await
    }

    /// The offline provider is a real backend provider, not a frontend fake.
    /// It is persisted so foreign keys such as `embeddings.provider_id` are
    /// satisfied, but it is hidden from the user-managed provider list.
    async fn ensure_mock_provider(&self) -> Result<(), AppError> {
        provider_repo::upsert_provider(&self.pool, &mock_config()).await
    }

    /// The cloud provider is virtual but still needs a row in `providers`
    /// because embeddings reference `providers(id)`.
    async fn ensure_cloud_provider(&self) -> Result<(), AppError> {
        provider_repo::upsert_provider(&self.pool, &self.cloud_config()).await
    }

    pub async fn default_chat_provider(
        &self,
    ) -> Result<(ProviderConfig, Arc<dyn ChatProvider>), AppError> {
        match self.ai_mode().await? {
            AiMode::Offline => self.mock_chat_provider().await,
            AiMode::Byok => self.byok_or_mock_chat_provider().await,
            AiMode::Cloud => {
                if let Some(cloud) = &self.cloud {
                    if self.cloud_ai_ready(cloud).await {
                        self.ensure_cloud_provider().await?;
                        return Ok((
                            self.cloud_config(),
                            Arc::new(CloudAiProvider::new(
                                cloud.clone(),
                                self.http.clone(),
                                CLOUD_CHAT_MODEL,
                                CLOUD_EMBEDDING_MODEL,
                            )),
                        ));
                    }
                    tracing::warn!(
                        "Verruckt Cloud AI is not currently usable; falling back to the next available provider"
                    );
                }
                self.byok_or_mock_chat_provider().await
            }
        }
    }

    pub async fn default_embedding_provider(
        &self,
    ) -> Result<(ProviderConfig, Arc<dyn EmbeddingProvider>), AppError> {
        match self.ai_mode().await? {
            AiMode::Offline => self.mock_embedding_provider().await,
            AiMode::Byok => self.byok_or_mock_embedding_provider().await,
            AiMode::Cloud => {
                if let Some(cloud) = &self.cloud {
                    if self.cloud_ai_ready(cloud).await {
                        self.ensure_cloud_provider().await?;
                        return Ok((
                            self.cloud_config(),
                            Arc::new(CloudAiProvider::new(
                                cloud.clone(),
                                self.http.clone(),
                                CLOUD_CHAT_MODEL,
                                CLOUD_EMBEDDING_MODEL,
                            )),
                        ));
                    }
                }
                self.byok_or_mock_embedding_provider().await
            }
        }
    }

    pub async fn is_mock_active(&self) -> Result<bool, AppError> {
        match self.ai_mode().await? {
            AiMode::Offline => Ok(true),
            AiMode::Cloud => Ok(self.cloud.is_none()),
            AiMode::Byok => {
                let provider = provider_repo::first_enabled_provider(&self.pool).await?;
                match provider {
                    None => Ok(true),
                    Some(provider) => Ok(self.api_key(&provider).await.is_err()),
                }
            }
        }
    }

    pub async fn ai_mode(&self) -> Result<AiMode, AppError> {
        let value: Option<String> = sqlx::query_scalar(
            "SELECT value_json FROM app_settings WHERE key = 'ai_provider_mode'",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(value
            .and_then(|value| serde_json::from_str::<String>(&value).ok())
            .map(|value| AiMode::parse(&value))
            // Preserve the pre-integration behaviour: use a configured BYOK
            // provider when present, otherwise the offline mock.
            .unwrap_or(AiMode::Byok))
    }

    async fn cloud_ai_ready(&self, cloud: &CloudAccountClient) -> bool {
        match cloud.cached_entitlements().await {
            Ok(entitlements) if entitlements.feature("official_ai") => {
                let limit = entitlements.limit("monthly_ai_credits");
                limit <= 0 || entitlements.usage.monthly_ai_credits_used < limit
            }
            _ => false,
        }
    }

    fn cloud_config(&self) -> ProviderConfig {
        let now = utc_now();
        ProviderConfig {
            id: CLOUD_PROVIDER_ID,
            display_name: "Verruckt Cloud".to_owned(),
            base_url: self
                .cloud
                .as_ref()
                .map(|cloud| cloud.base_url())
                .unwrap_or_else(|| "http://127.0.0.1:8080".to_owned()),
            chat_model: CLOUD_CHAT_MODEL.to_owned(),
            embedding_model: CLOUD_EMBEDDING_MODEL.to_owned(),
            api_key_ref: None,
            protocol: ProviderProtocol::ChatCompletions,
            enabled: true,
            created_at: now,
            updated_at: now,
        }
    }

    async fn mock_chat_provider(
        &self,
    ) -> Result<(ProviderConfig, Arc<dyn ChatProvider>), AppError> {
        self.ensure_mock_provider().await?;
        Ok((mock_config(), Arc::new(MockProvider::new())))
    }

    async fn mock_embedding_provider(
        &self,
    ) -> Result<(ProviderConfig, Arc<dyn EmbeddingProvider>), AppError> {
        self.ensure_mock_provider().await?;
        Ok((mock_config(), Arc::new(MockProvider::new())))
    }

    async fn byok_or_mock_chat_provider(
        &self,
    ) -> Result<(ProviderConfig, Arc<dyn ChatProvider>), AppError> {
        match provider_repo::first_enabled_provider(&self.pool).await? {
            None => self.mock_chat_provider().await,
            Some(provider) => match self.chat_provider(&provider).await {
                Ok(chat) => Ok((provider, chat)),
                Err(AppError::ProviderAuth) => {
                    tracing::warn!(
                        provider = %provider.id,
                        "provider has no usable API key; using the offline mock provider"
                    );
                    self.mock_chat_provider().await
                }
                Err(error) => Err(error),
            },
        }
    }

    async fn byok_or_mock_embedding_provider(
        &self,
    ) -> Result<(ProviderConfig, Arc<dyn EmbeddingProvider>), AppError> {
        match provider_repo::first_enabled_provider(&self.pool).await? {
            None => self.mock_embedding_provider().await,
            Some(provider) => match self.embedding_provider(&provider).await {
                Ok(embedding) => Ok((provider, embedding)),
                Err(AppError::ProviderAuth) => self.mock_embedding_provider().await,
                Err(error) => Err(error),
            },
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

    async fn load_capability_cache(
        &self,
        provider_id: Uuid,
    ) -> Result<Option<ProviderCapabilities>, AppError> {
        let value: Option<String> = sqlx::query_scalar(
            "SELECT capabilities_json FROM provider_capability_cache WHERE provider_id = ?",
        )
        .bind(provider_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(value.and_then(|value| serde_json::from_str(&value).ok()))
    }

    async fn save_capability_cache(
        &self,
        provider_id: Uuid,
        capabilities: &ProviderCapabilities,
    ) -> Result<(), AppError> {
        let json = serde_json::to_string(capabilities)
            .map_err(|error| AppError::Internal(error.to_string()))?;
        let protocol = capabilities
            .protocol
            .unwrap_or(ProviderProtocol::ChatCompletions)
            .as_str();
        sqlx::query(
            "INSERT INTO provider_capability_cache (provider_id, capabilities_json, protocol, probed_at) \
             VALUES (?, ?, ?, ?) ON CONFLICT(provider_id) DO UPDATE SET \
             capabilities_json = excluded.capabilities_json, protocol = excluded.protocol, probed_at = excluded.probed_at",
        )
        .bind(provider_id)
        .bind(json)
        .bind(protocol)
        .bind(utc_now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn clear_capability_cache(&self, provider_id: Uuid) -> Result<(), AppError> {
        sqlx::query("DELETE FROM provider_capability_cache WHERE provider_id = ?")
            .bind(provider_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

fn validate_provider_input(input: &ProviderUpsert) -> Result<(), AppError> {
    if input.display_name.trim().is_empty() {
        return Err(AppError::InvalidInput(
            "provider display name is required".into(),
        ));
    }
    normalize_base_url(&input.base_url)?;
    if input.chat_model.trim().is_empty() {
        return Err(AppError::InvalidInput("chat model is required".into()));
    }
    if input.embedding_model.trim().is_empty() {
        return Err(AppError::InvalidInput("embedding model is required".into()));
    }
    Ok(())
}

pub const CLOUD_PROVIDER_ID: Uuid = Uuid::from_u128(0x7665_7272_7563_6b74_0000_0000_0000_0002);
pub const CLOUD_CHAT_MODEL: &str = "verruckt-fast";
pub const CLOUD_EMBEDDING_MODEL: &str = "verruckt-embedding";

pub const MOCK_PROVIDER_ID: Uuid = Uuid::from_u128(0x7665_7272_7563_6b74_0000_0000_0000_0001);

pub fn mock_config() -> ProviderConfig {
    let now = utc_now();
    ProviderConfig {
        id: MOCK_PROVIDER_ID,
        display_name: "Offline Mock".to_owned(),
        base_url: "mock://local".to_owned(),
        chat_model: MOCK_MODEL.to_owned(),
        embedding_model: MOCK_MODEL.to_owned(),
        api_key_ref: None,
        protocol: ProviderProtocol::ChatCompletions,
        enabled: true,
        created_at: now,
        updated_at: now,
    }
}

pub fn is_mock_config(provider: &ProviderConfig) -> bool {
    provider.base_url.starts_with("mock://")
}
