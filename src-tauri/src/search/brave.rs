use super::traits::{SearchDocument, SearchProvider, SearchQuery, SearchResult};
use crate::error::AppError;
use async_trait::async_trait;
use reqwest::StatusCode;
use url::Url;

pub struct BraveSearchProvider {
    client: reqwest::Client,
    api_key: String,
}

impl BraveSearchProvider {
    pub fn new(client: reqwest::Client, api_key: impl Into<String>) -> Self {
        Self {
            client,
            api_key: api_key.into(),
        }
    }
}

#[async_trait]
impl SearchProvider for BraveSearchProvider {
    fn name(&self) -> &'static str {
        "brave"
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, AppError> {
        if query.query.trim().is_empty() {
            return Err(AppError::InvalidInput("search query is empty".into()));
        }
        let mut url = Url::parse("https://api.search.brave.com/res/v1/web/search")
            .map_err(|error| AppError::Internal(error.to_string()))?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("q", query.query.trim());
            pairs.append_pair("count", &query.count.clamp(1, 20).to_string());
            pairs.append_pair(
                "safesearch",
                if query.safe_search { "strict" } else { "off" },
            );
            if let Some(country) = query.country.as_deref().filter(|value| !value.is_empty()) {
                pairs.append_pair("country", country);
            }
        }

        let response = self
            .client
            .get(url)
            .header("Accept", "application/json")
            .header("X-Subscription-Token", &self.api_key)
            .send()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
            return Err(AppError::Search("search API authentication failed".into()));
        }
        if !status.is_success() {
            return Err(AppError::Search(format!(
                "brave search returned {status}: {}",
                String::from_utf8_lossy(&bytes)
                    .chars()
                    .take(300)
                    .collect::<String>()
            )));
        }

        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|error| AppError::Search(format!("invalid search response: {error}")))?;
        let results = value
            .get("web")
            .and_then(|web| web.get("results"))
            .and_then(|results| results.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(results
            .into_iter()
            .filter_map(|item| {
                let title = item.get("title")?.as_str()?.to_owned();
                let url = item.get("url")?.as_str()?.to_owned();
                let snippet = item
                    .get("description")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_owned();
                let published_at = item
                    .get("page_age")
                    .and_then(|value| value.as_str())
                    .map(str::to_owned);
                Some(SearchResult {
                    title,
                    url,
                    snippet,
                    source: "brave".into(),
                    published_at,
                })
            })
            .collect())
    }

    async fn fetch_document(&self, url: &str) -> Result<SearchDocument, AppError> {
        let parsed = Url::parse(url)
            .map_err(|error| AppError::InvalidInput(format!("invalid document URL: {error}")))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(AppError::InvalidInput(
                "document URL must use http or https".into(),
            ));
        }
        let response = self
            .client
            .get(parsed.clone())
            .header("Accept", "text/html, text/plain, application/pdf, */*")
            .send()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(AppError::Search(format!(
                "document fetch returned {status}"
            )));
        }
        let mime_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let bytes = response
            .bytes()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        Ok(SearchDocument {
            title: parsed
                .host_str()
                .map(str::to_owned)
                .unwrap_or_else(|| url.to_owned()),
            url: parsed.to_string(),
            text: String::from_utf8_lossy(&bytes).into_owned(),
            mime_type,
        })
    }
}
