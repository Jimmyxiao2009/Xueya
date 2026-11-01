use super::traits::{SearchDocument, SearchProvider, SearchQuery, SearchResult};
use crate::error::AppError;
use async_trait::async_trait;
use serde::Deserialize;

pub struct StepSearchProvider {
    client: reqwest::Client,
    api_key: String,
}
impl StepSearchProvider {
    pub fn new(client: reqwest::Client, api_key: String) -> Self {
        Self { client, api_key }
    }
}
#[derive(Deserialize)]
struct Response {
    results: Vec<Item>,
}
#[derive(Deserialize)]
struct Item {
    title: String,
    url: String,
    #[serde(default)]
    snippet: String,
}

#[async_trait]
impl SearchProvider for StepSearchProvider {
    fn name(&self) -> &'static str {
        "step"
    }
    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, AppError> {
        if query.query.trim().is_empty() {
            return Err(AppError::InvalidInput("search query is empty".into()));
        }
        let response = self
            .client
            .post("https://api.stepfun.com/v1/search")
            .bearer_auth(&self.api_key)
            .json(&serde_json::json!({"query":query.query,"n":query.count.clamp(1,20)}))
            .send()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?;
        if !response.status().is_success() {
            return Err(AppError::Search(format!(
                "网页搜索暂不可用（{}），请稍后重试",
                response.status()
            )));
        }
        let data: Response = response
            .json()
            .await
            .map_err(|e| AppError::Search(e.to_string()))?;
        Ok(data
            .results
            .into_iter()
            .filter(|r| {
                url::Url::parse(&r.url).is_ok_and(|u| matches!(u.scheme(), "https" | "http"))
            })
            .map(|r| SearchResult {
                title: r.title,
                url: r.url,
                snippet: r.snippet,
                source: "step".into(),
                published_at: None,
            })
            .collect())
    }
    async fn fetch_document(&self, _url: &str) -> Result<SearchDocument, AppError> {
        // URL imports must go through IngestionService's checked fetcher.
        Err(AppError::Search("请通过资料导入读取网页".into()))
    }
}
