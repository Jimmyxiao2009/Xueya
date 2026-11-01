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
    /// Optional JSON Schema for providers that support native structured output.
    #[serde(default)]
    pub json_schema: Option<serde_json::Value>,
    /// Ask the provider to stream. `chat_stream` always sets this internally.
    #[serde(default)]
    pub stream: bool,
}

impl ChatRequest {
    pub fn new(model: impl Into<String>, messages: Vec<ChatMessage>) -> Self {
        Self {
            model: model.into(),
            messages,
            temperature: 0.2,
            max_tokens: None,
            json_mode: false,
            json_schema: None,
            stream: false,
        }
    }

    pub fn json(model: impl Into<String>, messages: Vec<ChatMessage>) -> Self {
        Self {
            // Real reasoning models may spend part of the completion budget on
            // hidden reasoning before emitting the JSON object. Keep the
            // structured-output path from ending at an empty/partial response.
            max_tokens: Some(4096),
            json_mode: true,
            ..Self::new(model, messages)
        }
    }

    pub fn with_schema(mut self, name: impl Into<String>, schema: serde_json::Value) -> Self {
        self.json_schema = Some(serde_json::json!({
            "name": name.into(),
            "schema": schema,
            "strict": true,
        }));
        self
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingProbe {
    pub dimension: usize,
    pub model: String,
}

#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, AppError>;

    /// Probe with a minimal string and return the *actual* dimension the
    /// provider produced. No caller should ever assume 1536/3072/1024.
    async fn probe(&self) -> Result<EmbeddingProbe, AppError> {
        let vectors = self.embed(vec!["orbit-embedding-probe".to_owned()]).await?;
        let dimension = validate_embeddings(&vectors)?;
        Ok(EmbeddingProbe {
            dimension,
            model: "unknown".to_owned(),
        })
    }

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

/// Validate an embedding matrix: non-empty, all finite and all the same length.
pub fn validate_embeddings(vectors: &[Vec<f32>]) -> Result<usize, AppError> {
    let first = vectors
        .first()
        .ok_or_else(|| AppError::Embedding("embedding response was empty".into()))?;
    if first.is_empty() {
        return Err(AppError::Embedding("embedding vector is empty".into()));
    }
    let dimension = first.len();
    for (index, vector) in vectors.iter().enumerate() {
        if vector.is_empty() {
            return Err(AppError::Embedding(format!(
                "embedding vector {index} is empty"
            )));
        }
        if vector.len() != dimension {
            return Err(AppError::Embedding(format!(
                "embedding dimension mismatch: vector 0 has {dimension}, vector {index} has {}",
                vector.len()
            )));
        }
        if vector.iter().any(|value| !value.is_finite()) {
            return Err(AppError::Embedding(format!(
                "embedding vector {index} contains NaN or Infinity"
            )));
        }
    }
    Ok(dimension)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_non_finite_and_ragged_embeddings() {
        assert!(validate_embeddings(&[]).is_err());
        assert!(validate_embeddings(&[vec![]]).is_err());
        assert!(validate_embeddings(&[vec![1.0, f32::NAN]]).is_err());
        assert!(validate_embeddings(&[vec![1.0, 2.0], vec![3.0]]).is_err());
        assert_eq!(
            validate_embeddings(&[vec![1.0, 2.0], vec![3.0, 4.0]]).unwrap(),
            2
        );
    }
}
