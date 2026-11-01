pub mod brave;
pub mod searxng;
pub mod serper;
pub mod step;
pub mod tavily;
pub mod traits;

use crate::error::AppError;
use brave::BraveSearchProvider;
use futures_util::future::join_all;
use searxng::SearxngSearchProvider;
use serper::SerperSearchProvider;
use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use tavily::TavilySearchProvider;
use traits::{SearchProvider, SearchQuery, SearchResult};

#[derive(Clone)]
pub struct SearchProviderRegistry {
    client: reqwest::Client,
    providers: Arc<RwLock<Vec<Arc<dyn SearchProvider>>>>,
}

impl SearchProviderRegistry {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            providers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    fn upsert(&self, provider: Arc<dyn SearchProvider>) -> Result<(), AppError> {
        let name = provider.name();
        let mut providers = self
            .providers
            .write()
            .map_err(|_| AppError::Internal("search registry poisoned".into()))?;
        providers.retain(|existing| existing.name() != name);
        providers.push(provider);
        Ok(())
    }

    pub fn configure_step(&self, api_key: String) -> Result<(), AppError> {
        if api_key.trim().is_empty() {
            return Err(AppError::InvalidInput("Step search API key is empty".into()));
        }
        self.upsert(Arc::new(step::StepSearchProvider::new(
            self.client.clone(),
            api_key,
        )))
    }

    pub fn configure_brave(&self, api_key: impl Into<String>) -> Result<(), AppError> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(AppError::InvalidInput("Brave API key is empty".into()));
        }
        self.upsert(Arc::new(BraveSearchProvider::new(
            self.client.clone(),
            api_key,
        )))
    }

    pub fn configure_tavily(&self, api_key: impl Into<String>) -> Result<(), AppError> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(AppError::InvalidInput("Tavily API key is empty".into()));
        }
        self.upsert(Arc::new(TavilySearchProvider::new(
            self.client.clone(),
            api_key,
        )))
    }

    pub fn configure_serper(&self, api_key: impl Into<String>) -> Result<(), AppError> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(AppError::InvalidInput("Serper API key is empty".into()));
        }
        self.upsert(Arc::new(SerperSearchProvider::new(
            self.client.clone(),
            api_key,
        )))
    }

    pub fn configure_searxng(&self, base_url: impl Into<String>) -> Result<(), AppError> {
        let base_url = base_url.into();
        if base_url.trim().is_empty() {
            return Err(AppError::InvalidInput("SearXNG URL is empty".into()));
        }
        self.upsert(Arc::new(SearxngSearchProvider::new(
            self.client.clone(),
            base_url,
        )))
    }

    pub fn clear(&self) -> Result<(), AppError> {
        self.providers
            .write()
            .map_err(|_| AppError::Internal("search registry poisoned".into()))?
            .clear();
        Ok(())
    }

    pub fn is_configured(&self) -> bool {
        self.providers
            .read()
            .map(|providers| !providers.is_empty())
            .unwrap_or(false)
    }

    fn configured(&self) -> Result<Vec<Arc<dyn SearchProvider>>, AppError> {
        self.providers
            .read()
            .map(|providers| providers.clone())
            .map_err(|_| AppError::Internal("search registry poisoned".into()))
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, AppError> {
        let providers = self.configured()?;
        if providers.is_empty() {
            return Err(AppError::Search(
                "no search provider configured; manual URL/file import is still available".into(),
            ));
        }
        if providers.len() == 1 {
            return providers[0].search(query).await;
        }

        let batches = join_all(providers.iter().map(|provider| provider.search(query))).await;
        let mut sets = Vec::new();
        let mut first_error = None;
        for batch in batches {
            match batch {
                Ok(results) if !results.is_empty() => sets.push(results),
                Ok(_) => {}
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }
        if sets.is_empty() {
            return Err(first_error.unwrap_or_else(|| {
                AppError::Search("all configured search providers returned no results".into())
            }));
        }

        // Round-robin the providers so a broad query does not let one source
        // dominate the candidate pool before the optional LLM ranking step.
        let mut merged = Vec::new();
        let mut seen = HashSet::new();
        let mut indices = vec![0usize; sets.len()];
        loop {
            let mut progressed = false;
            for (index, set) in sets.iter().enumerate() {
                if let Some(result) = set.get(indices[index]) {
                    progressed = true;
                    indices[index] += 1;
                    if seen.insert(result.url.clone()) {
                        merged.push(result.clone());
                    }
                }
            }
            if !progressed {
                break;
            }
        }
        merged.truncate(query.count.clamp(10, 30));
        Ok(merged)
    }

    pub async fn fetch_document(&self, url: &str) -> Result<traits::SearchDocument, AppError> {
        let providers = self.configured()?;
        if providers.is_empty() {
            return Err(AppError::Search(
                "no search provider configured; manual URL/file import is still available".into(),
            ));
        }
        let mut last_error = None;
        for provider in providers {
            match provider.fetch_document(url).await {
                Ok(document) => return Ok(document),
                Err(error) => last_error = Some(error),
            }
        }
        Err(last_error
            .unwrap_or_else(|| AppError::Search("no search provider could fetch the document".into())))
    }
}