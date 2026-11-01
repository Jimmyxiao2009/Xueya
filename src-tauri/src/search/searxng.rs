use super::serper::fetch_generic_document;
use super::traits::{SearchDocument, SearchProvider, SearchQuery, SearchResult};
use crate::error::AppError;
use async_trait::async_trait;
use serde::Deserialize;
use url::Url;

pub struct SearxngSearchProvider {
    client: reqwest::Client,
    base_url: String,
}

impl SearxngSearchProvider {
    pub fn new(client: reqwest::Client, base_url: impl Into<String>) -> Self {
        Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_owned(),
        }
    }
}

#[derive(Deserialize)]
struct SearxngResponse {
    #[serde(default)]
    results: Vec<SearxngItem>,
}

#[derive(Deserialize)]
struct SearxngItem {
    title: String,
    url: String,
    #[serde(default)]
    content: String,
    #[serde(default, rename = "publishedDate")]
    published_date: Option<String>,
}

#[async_trait]
impl SearchProvider for SearxngSearchProvider {
    fn name(&self) -> &'static str {
        "searxng"
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, AppError> {
        if query.query.trim().is_empty() {
            return Err(AppError::InvalidInput("search query is empty".into()));
        }
        let endpoint = format!("{}/search", self.base_url);
        let parsed = Url::parse(&endpoint)
            .map_err(|error| AppError::InvalidInput(format!("invalid SearXNG URL: {error}")))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(AppError::InvalidInput(
                "SearXNG URL must use http or https".into(),
            ));
        }
        let response = self
            .client
            .get(parsed)
            .query(&[
                ("q", query.query.trim()),
                ("format", "json"),
                ("safesearch", if query.safe_search { "1" } else { "0" }),
            ])
            .send()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(AppError::Search(format!(
                "SearXNG search returned {status}"
            )));
        }
        let data: SearxngResponse = response
            .json()
            .await
            .map_err(|error| AppError::Search(format!("invalid search response: {error}")))?;
        Ok(data
            .results
            .into_iter()
            .filter(|item| {
                Url::parse(&item.url).is_ok_and(|url| matches!(url.scheme(), "https" | "http"))
            })
            .map(|item| SearchResult {
                title: item.title,
                url: item.url,
                snippet: item.content,
                source: "searxng".into(),
                published_at: item.published_date,
            })
            .collect())
    }

    async fn fetch_document(&self, url: &str) -> Result<SearchDocument, AppError> {
        fetch_generic_document(&self.client, url).await
    }
}