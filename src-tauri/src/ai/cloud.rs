use super::openai_compatible::OpenAiCompatibleProvider;
use super::traits::{
    ChatProvider, ChatRequest, ChatResponse, ChatStream, EmbeddingProbe, EmbeddingProvider,
};
use crate::cloud::CloudAccountClient;
use crate::domain::ProviderProtocol;
use crate::error::AppError;
use async_trait::async_trait;
use std::sync::Arc;

/// The Verruckt Cloud official-AI provider.
///
/// It deliberately reuses `OpenAiCompatibleProvider` for the wire format: the
/// cloud gateway is a Chat Completions + Embeddings relay, so a second protocol
/// implementation would only duplicate parsing. The difference is the
/// credential lifecycle. Every call asks `CloudAccountClient` for a valid
/// access token; a 401 triggers the client's single-flight refresh and the
/// original call is retried exactly once.
pub struct CloudAiProvider {
    cloud: Arc<CloudAccountClient>,
    http: reqwest::Client,
    chat_model: String,
    embedding_model: String,
}

impl CloudAiProvider {
    pub fn new(
        cloud: Arc<CloudAccountClient>,
        http: reqwest::Client,
        chat_model: impl Into<String>,
        embedding_model: impl Into<String>,
    ) -> Self {
        Self {
            cloud,
            http,
            chat_model: chat_model.into(),
            embedding_model: embedding_model.into(),
        }
    }

    async fn compatible(&self) -> Result<OpenAiCompatibleProvider, AppError> {
        let token = self.cloud.access_token().await?;
        Ok(OpenAiCompatibleProvider::with_protocol(
            self.http.clone(),
            self.cloud.base_url(),
            token,
            self.chat_model.clone(),
            self.embedding_model.clone(),
            ProviderProtocol::ChatCompletions,
        ))
    }

    async fn ensure_available(&self) -> Result<(), AppError> {
        let entitlements = self.cloud.cached_entitlements().await?;
        if !entitlements.feature("official_ai") {
            return Err(AppError::CloudEntitlementRequired);
        }
        let limit = entitlements.limit("monthly_ai_credits");
        if limit <= 0 || entitlements.usage.monthly_ai_credits_used >= limit {
            return Err(AppError::CloudQuotaExceeded);
        }
        Ok(())
    }
}

fn map_cloud_provider_error(error: AppError) -> AppError {
    match error {
        AppError::ProviderAuth => AppError::CloudAuth("the cloud access token was rejected".into()),
        AppError::Provider(message) => {
            let lower = message.to_ascii_lowercase();
            if lower.contains("402") || lower.contains("ai_quota_exceeded") {
                AppError::CloudQuotaExceeded
            } else if lower.contains("403") || lower.contains("entitlement_required") {
                AppError::CloudEntitlementRequired
            } else if lower.contains("409") || lower.contains("sync_conflict") {
                AppError::CloudSyncConflict(message)
            } else {
                AppError::CloudUnavailable(message)
            }
        }
        other => other,
    }
}

#[async_trait]
impl ChatProvider for CloudAiProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, AppError> {
        self.ensure_available().await?;
        let provider = self.compatible().await?;
        match provider.chat(request.clone()).await {
            Ok(response) => Ok(response),
            Err(AppError::ProviderAuth) => {
                self.cloud.force_refresh().await?;
                let provider = self.compatible().await?;
                provider
                    .chat(request)
                    .await
                    .map_err(map_cloud_provider_error)
            }
            Err(error) => Err(map_cloud_provider_error(error)),
        }
    }

    async fn test_connection(&self) -> Result<(), AppError> {
        self.ensure_available().await?;
        let provider = self.compatible().await?;
        match provider.test_connection().await {
            Ok(()) => Ok(()),
            Err(AppError::ProviderAuth) => {
                self.cloud.force_refresh().await?;
                let provider = self.compatible().await?;
                provider
                    .test_connection()
                    .await
                    .map_err(map_cloud_provider_error)
            }
            Err(error) => Err(map_cloud_provider_error(error)),
        }
    }

    async fn chat_stream(&self, request: ChatRequest) -> Result<ChatStream, AppError> {
        self.ensure_available().await?;
        let provider = self.compatible().await?;
        match provider.chat_stream(request.clone()).await {
            Ok(stream) => Ok(stream),
            Err(AppError::ProviderAuth) => {
                self.cloud.force_refresh().await?;
                let provider = self.compatible().await?;
                provider
                    .chat_stream(request)
                    .await
                    .map_err(map_cloud_provider_error)
            }
            Err(error) => Err(map_cloud_provider_error(error)),
        }
    }
}

#[async_trait]
impl EmbeddingProvider for CloudAiProvider {
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, AppError> {
        self.ensure_available().await?;
        let provider = self.compatible().await?;
        match provider.embed(texts.clone()).await {
            Ok(vectors) => Ok(vectors),
            Err(AppError::ProviderAuth) => {
                self.cloud.force_refresh().await?;
                let provider = self.compatible().await?;
                provider
                    .embed(texts)
                    .await
                    .map_err(map_cloud_provider_error)
            }
            Err(error) => Err(map_cloud_provider_error(error)),
        }
    }

    async fn probe(&self) -> Result<EmbeddingProbe, AppError> {
        self.ensure_available().await?;
        let provider = self.compatible().await?;
        match provider.probe().await {
            Ok(probe) => Ok(probe),
            Err(AppError::ProviderAuth) => {
                self.cloud.force_refresh().await?;
                let provider = self.compatible().await?;
                provider.probe().await.map_err(map_cloud_provider_error)
            }
            Err(error) => Err(map_cloud_provider_error(error)),
        }
    }
}
