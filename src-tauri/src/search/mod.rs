pub mod brave;
pub mod traits;

use crate::error::AppError;
use brave::BraveSearchProvider;
use std::sync::{Arc, RwLock};
use traits::{SearchProvider, SearchQuery, SearchResult};

#[derive(Clone)]
pub struct SearchProviderRegistry {
    client: reqwest::Client,
    provider: Arc<RwLock<Option<Arc<dyn SearchProvider>>>>,
}

impl SearchProviderRegistry {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            provider: Arc::new(RwLock::new(None)),
        }
    }

    pub fn configure_brave(&self, api_key: impl Into<String>) -> Result<(), AppError> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(AppError::InvalidInput("Brave API key is empty".into()));
        }
        let provider: Arc<dyn SearchProvider> =
            Arc::new(BraveSearchProvider::new(self.client.clone(), api_key));
        *self
            .provider
            .write()
            .map_err(|_| AppError::Internal("search provider registry poisoned".into()))? =
            Some(provider);
        Ok(())
    }

    pub fn clear(&self) -> Result<(), AppError> {
        *self
            .provider
            .write()
            .map_err(|_| AppError::Internal("search provider registry poisoned".into()))? = None;
        Ok(())
    }

    pub fn is_configured(&self) -> bool {
        self.provider
            .read()
            .map(|value| value.is_some())
            .unwrap_or(false)
    }

    fn current(&self) -> Result<Arc<dyn SearchProvider>, AppError> {
        self.provider
            .read()
            .map_err(|_| AppError::Internal("search provider registry poisoned".into()))?
            .clone()
            .ok_or_else(|| {
                AppError::Search(
                    "no search provider configured; manual URL/file import is still available"
                        .into(),
                )
            })
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, AppError> {
        self.current()?.search(query).await
    }

    pub async fn fetch_document(&self, url: &str) -> Result<traits::SearchDocument, AppError> {
        self.current()?.fetch_document(url).await
    }
}
