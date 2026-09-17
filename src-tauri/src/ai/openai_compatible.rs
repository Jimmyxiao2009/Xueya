use super::traits::{
    ChatMessage, ChatProvider, ChatRequest, ChatResponse, ChatUsage, EmbeddingProvider,
};
use crate::error::AppError;
use async_trait::async_trait;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;

pub struct OpenAiCompatibleProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    chat_model: String,
    embedding_model: String,
}

impl OpenAiCompatibleProvider {
    pub fn new(
        client: reqwest::Client,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        chat_model: impl Into<String>,
        embedding_model: impl Into<String>,
    ) -> Self {
        Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            api_key: api_key.into(),
            chat_model: chat_model.into(),
            embedding_model: embedding_model.into(),
        }
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.client
            .request(method, self.endpoint(path))
            .bearer_auth(&self.api_key)
    }

    pub async fn test_connection(&self) -> Result<(), AppError> {
        let response = self
            .request(reqwest::Method::GET, "models")
            .send()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        match response.status() {
            status if status.is_success() => Ok(()),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(AppError::ProviderAuth),
            status => {
                let body = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "unreadable response body".into());
                Err(AppError::Provider(format!(
                    "models endpoint returned {status}: {}",
                    body.chars().take(500).collect::<String>()
                )))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct ApiChatResponse {
    choices: Vec<ApiChoice>,
    model: Option<String>,
    usage: Option<ApiUsage>,
}

#[derive(Debug, Deserialize)]
struct ApiChoice {
    message: ApiMessage,
}

#[derive(Debug, Deserialize)]
struct ApiMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiUsage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    total_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ApiEmbeddingResponse {
    data: Vec<ApiEmbedding>,
}

#[derive(Debug, Deserialize)]
struct ApiEmbedding {
    embedding: Vec<f32>,
}

fn parse_chat_response(body: &[u8], fallback_model: &str) -> Result<ChatResponse, AppError> {
    let response: ApiChatResponse = serde_json::from_slice(body)
        .map_err(|error| AppError::AiResponseInvalid(error.to_string()))?;
    let choice = response
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| AppError::AiResponseInvalid("chat response had no choices".into()))?;
    let content = choice
        .message
        .content
        .ok_or_else(|| AppError::AiResponseInvalid("chat response had no content".into()))?;
    Ok(ChatResponse {
        content,
        model: response.model.unwrap_or_else(|| fallback_model.to_owned()),
        usage: response.usage.map(|usage| ChatUsage {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        }),
    })
}

fn parse_embedding_response(body: &[u8]) -> Result<Vec<Vec<f32>>, AppError> {
    let response: ApiEmbeddingResponse =
        serde_json::from_slice(body).map_err(|error| AppError::Embedding(error.to_string()))?;
    if response.data.is_empty() {
        return Err(AppError::Embedding("embedding response had no data".into()));
    }
    Ok(response
        .data
        .into_iter()
        .map(|item| item.embedding)
        .collect())
}

#[async_trait]
impl ChatProvider for OpenAiCompatibleProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, AppError> {
        let model = if request.model.trim().is_empty() {
            self.chat_model.clone()
        } else {
            request.model
        };
        let mut body = json!({
            "model": model,
            "messages": request.messages,
            "temperature": request.temperature,
        });
        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }
        if request.json_mode {
            body["response_format"] = json!({ "type": "json_object" });
        }

        let response = self
            .request(reqwest::Method::POST, "chat/completions")
            .json(&body)
            .send()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
            return Err(AppError::ProviderAuth);
        }
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(AppError::Provider(format!(
                "chat endpoint returned {status}: {}",
                body.chars().take(500).collect::<String>()
            )));
        }
        parse_chat_response(&bytes, &self.chat_model)
    }
}

#[async_trait]
impl EmbeddingProvider for OpenAiCompatibleProvider {
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, AppError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let body = json!({
            "model": self.embedding_model,
            "input": texts,
        });
        let response = self
            .request(reqwest::Method::POST, "embeddings")
            .json(&body)
            .send()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
            return Err(AppError::ProviderAuth);
        }
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(AppError::Embedding(format!(
                "embedding endpoint returned {status}: {}",
                body.chars().take(500).collect::<String>()
            )));
        }
        parse_embedding_response(&bytes)
    }
}

pub fn compact_messages(messages: &[ChatMessage]) -> Vec<ChatMessage> {
    messages.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chat_response() {
        let body = br#"{"model":"test-model","choices":[{"message":{"content":"hello"}}],"usage":{"total_tokens":7}}"#;
        let response = parse_chat_response(body, "fallback").expect("parse");
        assert_eq!(response.content, "hello");
        assert_eq!(response.model, "test-model");
        assert_eq!(response.usage.and_then(|usage| usage.total_tokens), Some(7));
    }

    #[test]
    fn parses_embedding_response() {
        let body = br#"{"data":[{"embedding":[0.1,0.2]},{"embedding":[0.3,0.4]}]}"#;
        let vectors = parse_embedding_response(body).expect("parse");
        assert_eq!(vectors.len(), 2);
        assert_eq!(vectors[0], vec![0.1, 0.2]);
    }
}
