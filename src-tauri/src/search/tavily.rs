use super::traits::{SearchDocument, SearchProvider, SearchQuery, SearchResult};
use crate::error::AppError;
use async_trait::async_trait;
use reqwest::StatusCode;
use serde::Deserialize;
use url::Url;

pub struct TavilySearchProvider {
    client: reqwest::Client,
    api_key: String,
}

impl TavilySearchProvider {
    pub fn new(client: reqwest::Client, api_key: impl Into<String>) -> Self {
        Self {
            client,
            api_key: api_key.into(),
        }
    }
}

#[derive(Deserialize)]
struct TavilyResponse {
    #[serde(default)]
    results: Vec<TavilyItem>,
}

#[derive(Deserialize)]
struct TavilyItem {
    title: String,
    url: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    published_date: Option<String>,
}

#[async_trait]
impl SearchProvider for TavilySearchProvider {
    fn name(&self) -> &'static str {
        "tavily"
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, AppError> {
        if query.query.trim().is_empty() {
            return Err(AppError::InvalidInput("search query is empty".into()));
        }
        let response = self
            .client
            .post("https://api.tavily.com/search")
            .json(&serde_json::json!({
                "api_key": self.api_key,
                "query": query.query.trim(),
                "max_results": query.count.clamp(1, 20),
                "search_depth": "basic",
                "include_answer": false,
                "include_raw_content": false,
            }))
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
                "tavily search returned {status}: {}",
                String::from_utf8_lossy(&bytes).chars().take(300).collect::<String>()
            )));
        }
        let data: TavilyResponse = serde_json::from_slice(&bytes)
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
                source: "tavily".into(),
                published_at: item.published_date,
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