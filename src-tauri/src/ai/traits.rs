use crate::error::AppError;
use async_trait::async_trait;
use futures_util::stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

pub type ChatStream = Pin<Box<dyn futures_util::Stream<Item = Result<ChatDelta, AppError>> + Send>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: f32,
    pub max_tokens: Option<u32>,
    pub json_mode: bool,
}

impl ChatRequest {
    pub fn new(model: impl Into<String>, messages: Vec<ChatMessage>) -> Self {
        Self {
            model: model.into(),
            messages,
            temperature: 0.2,
            max_tokens: None,
            json_mode: false,
        }
    }

    pub fn json(model: impl Into<String>, messages: Vec<ChatMessage>) -> Self {
        Self {
            json_mode: true,
            ..Self::new(model, messages)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatUsage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub model: String,
    pub usage: Option<ChatUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatDelta {
    pub content: String,
    pub finish_reason: Option<String>,
}

#[async_trait]
pub trait ChatProvider: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, AppError>;

    async fn test_connection(&self) -> Result<(), AppError> {
        self.chat(ChatRequest::new(
            "",
            vec![ChatMessage::user("Reply with the single word ok.")],
        ))
        .await
        .map(|_| ())
    }

    async fn chat_stream(&self, request: ChatRequest) -> Result<ChatStream, AppError> {
        let response = self.chat(request).await?;
        Ok(Box::pin(stream::once(async move {
            Ok(ChatDelta {
                content: response.content,
                finish_reason: Some("stop".into()),
            })
        })))
    }
}

#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, AppError>;

    async fn embed_batch(
        &self,
        texts: Vec<String>,
        batch_size: usize,
    ) -> Result<Vec<Vec<f32>>, AppError> {
        if batch_size == 0 {
            return Err(AppError::InvalidInput("batch size must be positive".into()));
        }
        let mut output = Vec::with_capacity(texts.len());
        for batch in texts.chunks(batch_size) {
            let mut values = self.embed(batch.to_vec()).await?;
            output.append(&mut values);
        }
        Ok(output)
    }
}
