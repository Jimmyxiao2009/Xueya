use super::capabilities::ProviderCapabilities;
use super::endpoint::endpoint_url;
use super::http::{send_with_retry, RetryPolicy};
use super::traits::{
    validate_embeddings, ChatDelta, ChatMessage, ChatProvider, ChatRequest, ChatResponse,
    ChatStream, ChatUsage, EmbeddingProbe, EmbeddingProvider,
};
use crate::domain::ProviderProtocol;
use crate::error::AppError;
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::{stream, Stream, StreamExt};
use reqwest::{Response, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};
use std::pin::Pin;

pub struct OpenAiCompatibleProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    chat_model: String,
    embedding_model: String,
    protocol: ProviderProtocol,
    retry: RetryPolicy,
}

impl OpenAiCompatibleProvider {
    /// Backwards-compatible constructor. Pinned to Chat Completions, which is
    /// the broadest third-party OpenAI-compatible protocol.
    pub fn new(
        client: reqwest::Client,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        chat_model: impl Into<String>,
        embedding_model: impl Into<String>,
    ) -> Self {
        Self::with_protocol(
            client,
            base_url,
            api_key,
            chat_model,
            embedding_model,
            ProviderProtocol::ChatCompletions,
        )
    }

    pub fn with_protocol(
        client: reqwest::Client,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        chat_model: impl Into<String>,
        embedding_model: impl Into<String>,
        protocol: ProviderProtocol,
    ) -> Self {
        Self {
            client,
            base_url: base_url.into(),
            api_key: api_key.into(),
            chat_model: chat_model.into(),
            embedding_model: embedding_model.into(),
            protocol,
            retry: RetryPolicy::default(),
        }
    }

    pub fn with_retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = retry;
        self
    }

    pub fn protocol(&self) -> ProviderProtocol {
        self.protocol
    }

    fn endpoint(&self, resource: &str) -> Result<String, AppError> {
        endpoint_url(&self.base_url, resource)
    }

    fn request(
        &self,
        method: reqwest::Method,
        resource: &str,
    ) -> Result<reqwest::RequestBuilder, AppError> {
        Ok(self
            .client
            .request(method, self.endpoint(resource)?)
            .bearer_auth(&self.api_key))
    }

    fn model_or_default<'a>(&'a self, requested: &'a str) -> &'a str {
        if requested.trim().is_empty() {
            &self.chat_model
        } else {
            requested
        }
    }

    pub async fn probe_capabilities(&self) -> ProviderCapabilities {
        let mut capabilities = ProviderCapabilities {
            protocol: Some(self.protocol),
            ..ProviderCapabilities::default()
        };

        match self.probe_chat_completions().await {
            Ok(structured) => {
                capabilities.chat_completions = true;
                capabilities.structured_output = structured;
            }
            Err(error) => capabilities
                .probe_errors
                .push(format!("chat completions: {}", error.public_message())),
        }

        match self.probe_responses().await {
            Ok(()) => capabilities.responses = true,
            Err(error) => capabilities
                .probe_errors
                .push(format!("responses: {}", error.public_message())),
        }

        match self.probe_embeddings().await {
            Ok(dimension) => {
                capabilities.embeddings = true;
                capabilities.embedding_dimension = Some(dimension);
            }
            Err(error) => capabilities
                .probe_errors
                .push(format!("embeddings: {}", error.public_message())),
        }

        if capabilities.supports_chat() {
            match self.probe_streaming().await {
                Ok(supported) => capabilities.streaming = supported,
                Err(error) => capabilities
                    .probe_errors
                    .push(format!("streaming: {}", error.public_message())),
            }
        }

        capabilities.protocol = Some(if self.protocol == ProviderProtocol::Auto {
            capabilities.preferred_protocol()
        } else {
            self.protocol
        });
        capabilities
    }

    async fn probe_chat_completions(&self) -> Result<bool, AppError> {
        let plain = json!({
            "model": self.chat_model,
            "messages": [{"role": "user", "content": "ping"}],
            "max_tokens": 1,
            "temperature": 0.0,
        });
        let mut structured = json!(plain);
        structured["response_format"] = json!({ "type": "json_object" });

        let response = send_with_retry(
            || {
                self.request(reqwest::Method::POST, "chat/completions")
                    .expect("endpoint")
                    .json(&structured)
            },
            self.retry,
        )
        .await?;
        if response.status().is_success() {
            return Ok(true);
        }
        if matches!(
            response.status(),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) {
            return Err(AppError::ProviderAuth);
        }
        let status = response.status();
        if matches!(
            status,
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY
        ) {
            // Provider does not accept response_format: json_object. Plain chat
            // still means the endpoint is usable; structured output is not.
            let response = send_with_retry(
                || {
                    self.request(reqwest::Method::POST, "chat/completions")
                        .expect("endpoint")
                        .json(&plain)
                },
                self.retry,
            )
            .await?;
            if response.status().is_success() {
                return Ok(false);
            }
            return Err(self.status_error(response).await);
        }
        Err(self.status_error(response).await)
    }

    async fn probe_responses(&self) -> Result<(), AppError> {
        let body = json!({
            "model": self.chat_model,
            "input": "ping",
            "max_output_tokens": 8,
        });
        let response = send_with_retry(
            || {
                self.request(reqwest::Method::POST, "responses")
                    .expect("endpoint")
                    .json(&body)
            },
            self.retry,
        )
        .await?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(self.status_error(response).await)
        }
    }

    async fn probe_embeddings(&self) -> Result<usize, AppError> {
        let body = json!({
            "model": self.embedding_model,
            "input": ["orbit-embedding-probe"],
        });
        let response = send_with_retry(
            || {
                self.request(reqwest::Method::POST, "embeddings")
                    .expect("endpoint")
                    .json(&body)
            },
            self.retry,
        )
        .await?;
        if !response.status().is_success() {
            return Err(self.status_error(response).await);
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        let vectors = parse_embedding_response(&bytes)?;
        validate_embeddings(&vectors)
    }

    async fn probe_streaming(&self) -> Result<bool, AppError> {
        let body = json!({
            "model": self.chat_model,
            "messages": [{"role": "user", "content": "ping"}],
            "max_tokens": 1,
            "stream": true,
        });
        let response = send_with_retry(
            || {
                self.request(reqwest::Method::POST, "chat/completions")
                    .expect("endpoint")
                    .json(&body)
            },
            self.retry,
        )
        .await?;
        if !response.status().is_success() {
            return Err(self.status_error(response).await);
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_ascii_lowercase();
        Ok(content_type.contains("text/event-stream") || content_type.contains("stream"))
    }

    async fn status_error(&self, response: Response) -> AppError {
        let status = response.status();
        if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
            return AppError::ProviderAuth;
        }
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "unreadable response body".into());
        AppError::Provider(format!(
            "provider returned {status}: {}",
            body.chars().take(500).collect::<String>()
        ))
    }

    async fn chat_completions(&self, request: ChatRequest) -> Result<ChatResponse, AppError> {
        let model = self.model_or_default(&request.model).to_owned();
        let mut body = json!({
            "model": model,
            "messages": request.messages,
            "temperature": request.temperature,
        });
        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }
        if let Some(schema) = request.json_schema.clone() {
            body["response_format"] = json!({
                "type": "json_schema",
                "json_schema": schema,
            });
        } else if request.json_mode {
            body["response_format"] = json!({ "type": "json_object" });
        }

        let response = send_with_retry(
            || {
                self.request(reqwest::Method::POST, "chat/completions")
                    .expect("endpoint")
                    .json(&body)
            },
            self.retry,
        )
        .await?;

        if !response.status().is_success() {
            let status = response.status();
            if matches!(
                status,
                StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY
            ) && request.json_schema.is_some()
            {
                // Native JSON Schema is not universally supported. Fall back to
                // a plain JSON response once, then let structured generation's
                // own repair retry handle the rest.
                let mut fallback = body.clone();
                fallback["response_format"] = json!({ "type": "json_object" });
                let response = send_with_retry(
                    || {
                        self.request(reqwest::Method::POST, "chat/completions")
                            .expect("endpoint")
                            .json(&fallback)
                    },
                    self.retry,
                )
                .await?;
                if !response.status().is_success() {
                    return Err(self.status_error(response).await);
                }
                let bytes = response
                    .bytes()
                    .await
                    .map_err(|error| AppError::Network(error.to_string()))?;
                return parse_chat_response(&bytes, &self.chat_model);
            }
            return Err(self.status_error(response).await);
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        parse_chat_response(&bytes, &self.chat_model)
    }

    async fn chat_responses(&self, request: ChatRequest) -> Result<ChatResponse, AppError> {
        let model = self.model_or_default(&request.model).to_owned();
        let mut instructions = Vec::new();
        let mut input = Vec::new();
        for message in &request.messages {
            if message.role == "system" {
                instructions.push(message.content.clone());
            } else {
                input.push(json!({
                    "role": message.role,
                    "content": [{ "type": "input_text", "text": message.content }],
                }));
            }
        }
        let mut body = json!({
            "model": model,
            "input": input,
        });
        if !instructions.is_empty() {
            body["instructions"] = json!(instructions.join("\n\n"));
        }
        if let Some(max_tokens) = request.max_tokens {
            body["max_output_tokens"] = json!(max_tokens);
        }
        if let Some(schema) = request.json_schema.clone() {
            body["text"] = json!({
                "format": {
                    "type": "json_schema",
                    "name": schema.get("name").and_then(Value::as_str).unwrap_or("orbit_output"),
                    "schema": schema.get("schema").cloned().unwrap_or(Value::Null),
                    "strict": true,
                }
            });
        } else if request.json_mode {
            body["text"] = json!({ "format": { "type": "json_object" } });
        }

        let response = send_with_retry(
            || {
                self.request(reqwest::Method::POST, "responses")
                    .expect("endpoint")
                    .json(&body)
            },
            self.retry,
        )
        .await?;
        if !response.status().is_success() {
            return Err(self.status_error(response).await);
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        parse_responses_response(&bytes, &self.chat_model)
    }

    async fn chat_stream_impl(&self, request: ChatRequest) -> Result<ChatStream, AppError> {
        match self.protocol {
            ProviderProtocol::Responses => self.chat_stream_responses(request).await,
            _ => self.chat_stream_chat_completions(request).await,
        }
    }

    async fn chat_stream_chat_completions(
        &self,
        request: ChatRequest,
    ) -> Result<ChatStream, AppError> {
        let model = self.model_or_default(&request.model).to_owned();
        let mut body = json!({
            "model": model,
            "messages": request.messages,
            "temperature": request.temperature,
            "stream": true,
        });
        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }
        if request.json_mode {
            body["response_format"] = json!({ "type": "json_object" });
        }
        let response = send_with_retry(
            || {
                self.request(reqwest::Method::POST, "chat/completions")
                    .expect("endpoint")
                    .json(&body)
            },
            self.retry,
        )
        .await?;
        if !response.status().is_success() {
            return Err(self.status_error(response).await);
        }
        Ok(sse_stream(response, ProviderProtocol::ChatCompletions))
    }

    async fn chat_stream_responses(&self, request: ChatRequest) -> Result<ChatStream, AppError> {
        let model = self.model_or_default(&request.model).to_owned();
        let mut instructions = Vec::new();
        let mut input = Vec::new();
        for message in &request.messages {
            if message.role == "system" {
                instructions.push(message.content.clone());
            } else {
                input.push(json!({
                    "role": message.role,
                    "content": [{ "type": "input_text", "text": message.content }],
                }));
            }
        }
        let mut body = json!({ "model": model, "input": input, "stream": true });
        if !instructions.is_empty() {
            body["instructions"] = json!(instructions.join("\n\n"));
        }
        if let Some(max_tokens) = request.max_tokens {
            body["max_output_tokens"] = json!(max_tokens);
        }
        let response = send_with_retry(
            || {
                self.request(reqwest::Method::POST, "responses")
                    .expect("endpoint")
                    .json(&body)
            },
            self.retry,
        )
        .await?;
        if !response.status().is_success() {
            return Err(self.status_error(response).await);
        }
        Ok(sse_stream(response, ProviderProtocol::Responses))
    }
}

#[async_trait]
impl ChatProvider for OpenAiCompatibleProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, AppError> {
        match self.protocol {
            ProviderProtocol::Responses => self.chat_responses(request).await,
            _ => self.chat_completions(request).await,
        }
    }

    async fn test_connection(&self) -> Result<(), AppError> {
        let capabilities = self.probe_capabilities().await;
        if capabilities.supports_chat() {
            Ok(())
        } else {
            Err(AppError::Provider(
                capabilities
                    .probe_errors
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "provider has no usable chat endpoint".into()),
            ))
        }
    }

    async fn chat_stream(&self, request: ChatRequest) -> Result<ChatStream, AppError> {
        self.chat_stream_impl(request).await
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
        let response = send_with_retry(
            || {
                self.request(reqwest::Method::POST, "embeddings")
                    .expect("endpoint")
                    .json(&body)
            },
            self.retry,
        )
        .await?;
        if !response.status().is_success() {
            return Err(self.status_error(response).await);
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        parse_embedding_response(&bytes)
    }

    async fn probe(&self) -> Result<EmbeddingProbe, AppError> {
        let body = json!({
            "model": self.embedding_model,
            "input": ["orbit-embedding-probe"],
        });
        let response = send_with_retry(
            || {
                self.request(reqwest::Method::POST, "embeddings")
                    .expect("endpoint")
                    .json(&body)
            },
            self.retry,
        )
        .await?;
        if !response.status().is_success() {
            return Err(self.status_error(response).await);
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        let vectors = parse_embedding_response(&bytes)?;
        let dimension = validate_embeddings(&vectors)?;
        Ok(EmbeddingProbe {
            dimension,
            model: self.embedding_model.clone(),
        })
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

#[derive(Debug, Deserialize)]
struct ResponsesResponse {
    output: Vec<ResponsesOutput>,
    model: Option<String>,
    usage: Option<ResponsesUsage>,
}

#[derive(Debug, Deserialize)]
struct ResponsesOutput {
    #[serde(rename = "type")]
    kind: Option<String>,
    content: Option<Vec<ResponsesContent>>,
}

#[derive(Debug, Deserialize)]
struct ResponsesContent {
    #[serde(rename = "type")]
    kind: Option<String>,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponsesUsage {
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
    total_tokens: Option<u32>,
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

fn parse_responses_response(body: &[u8], fallback_model: &str) -> Result<ChatResponse, AppError> {
    let response: ResponsesResponse = serde_json::from_slice(body)
        .map_err(|error| AppError::AiResponseInvalid(error.to_string()))?;
    let mut content = String::new();
    for output in response.output {
        if let Some(parts) = output.content {
            for part in parts {
                if part.kind.as_deref() == Some("output_text") || part.text.is_some() {
                    if let Some(text) = part.text {
                        content.push_str(&text);
                    }
                }
            }
        } else if output.kind.as_deref() == Some("output_text") {
            // Some gateways flatten output text onto the output item itself.
            continue;
        }
    }
    if content.trim().is_empty() {
        return Err(AppError::AiResponseInvalid(
            "responses response had no output_text".into(),
        ));
    }
    Ok(ChatResponse {
        content,
        model: response.model.unwrap_or_else(|| fallback_model.to_owned()),
        usage: response.usage.map(|usage| ChatUsage {
            prompt_tokens: usage.input_tokens,
            completion_tokens: usage.output_tokens,
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
    let vectors: Vec<Vec<f32>> = response
        .data
        .into_iter()
        .map(|item| item.embedding)
        .collect();
    validate_embeddings(&vectors)?;
    Ok(vectors)
}

fn parse_sse_event(payload: &str, protocol: ProviderProtocol) -> Option<ChatDelta> {
    if payload.trim() == "[DONE]" {
        return None;
    }
    let value: Value = serde_json::from_str(payload).ok()?;
    match protocol {
        ProviderProtocol::Responses => {
            let kind = value
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if kind == "response.output_text.delta" {
                let text = value
                    .get("delta")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                return Some(ChatDelta {
                    content: text.to_owned(),
                    finish_reason: None,
                });
            }
            if kind == "response.completed" {
                return Some(ChatDelta {
                    content: String::new(),
                    finish_reason: Some("stop".into()),
                });
            }
            None
        }
        _ => {
            let delta = value
                .get("choices")
                .and_then(Value::as_array)
                .and_then(|choices| choices.first())
                .and_then(|choice| choice.get("delta"))?;
            let content = delta
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let finish_reason = value
                .get("choices")
                .and_then(Value::as_array)
                .and_then(|choices| choices.first())
                .and_then(|choice| choice.get("finish_reason"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            Some(ChatDelta {
                content,
                finish_reason,
            })
        }
    }
}

struct SseState {
    stream: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    buffer: Vec<u8>,
    done: bool,
    protocol: ProviderProtocol,
}

impl SseState {
    fn next_event(&mut self) -> Result<Option<ChatDelta>, AppError> {
        loop {
            let separator = self
                .buffer
                .windows(2)
                .position(|window| window == b"\n\n")
                .map(|index| (index, 2))
                .or_else(|| {
                    self.buffer
                        .windows(4)
                        .position(|window| window == b"\r\n\r\n")
                        .map(|index| (index, 4))
                });
            let Some((index, separator_len)) = separator else {
                return Ok(None);
            };
            let event: Vec<u8> = self.buffer.drain(..index + separator_len).collect();
            let text = String::from_utf8_lossy(&event);
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with(':') {
                    continue;
                }
                let Some(payload) = line.strip_prefix("data:") else {
                    continue;
                };
                let payload = payload.trim();
                if payload == "[DONE]" {
                    self.done = true;
                    continue;
                }
                if let Some(delta) = parse_sse_event(payload, self.protocol) {
                    return Ok(Some(delta));
                }
            }
            if self.done {
                return Ok(None);
            }
        }
    }
}

fn sse_stream(response: Response, protocol: ProviderProtocol) -> ChatStream {
    let stream: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>> =
        Box::pin(response.bytes_stream());
    let state = SseState {
        stream,
        buffer: Vec::new(),
        done: false,
        protocol,
    };
    Box::pin(stream::try_unfold(state, |mut state| async move {
        loop {
            if state.done && state.buffer.is_empty() {
                return Ok(None);
            }
            if let Some(event) = state.next_event()? {
                return Ok(Some((event, state)));
            }
            if state.done {
                return Ok(None);
            }
            match state.stream.next().await {
                Some(Ok(chunk)) => {
                    state.buffer.extend_from_slice(&chunk);
                }
                Some(Err(error)) => return Err(AppError::Network(error.to_string())),
                None => {
                    state.done = true;
                    if let Some(event) = state.next_event()? {
                        return Ok(Some((event, state)));
                    }
                    return Ok(None);
                }
            }
        }
    }))
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
    fn parses_responses_response() {
        let body = br#"{"model":"test-model","output":[{"type":"message","content":[{"type":"output_text","text":"hello"}]}],"usage":{"input_tokens":2,"output_tokens":3,"total_tokens":5}}"#;
        let response = parse_responses_response(body, "fallback").expect("parse");
        assert_eq!(response.content, "hello");
        assert_eq!(response.usage.and_then(|usage| usage.total_tokens), Some(5));
    }

    #[test]
    fn parses_embedding_response() {
        let body = br#"{"data":[{"embedding":[0.1,0.2]},{"embedding":[0.3,0.4]}]}"#;
        let vectors = parse_embedding_response(body).expect("parse");
        assert_eq!(vectors.len(), 2);
        assert_eq!(vectors[0], vec![0.1, 0.2]);
    }

    #[test]
    fn rejects_empty_embeddings() {
        let body = br#"{"data":[]}"#;
        assert!(parse_embedding_response(body).is_err());
    }

    #[test]
    fn parses_chat_completions_sse_delta() {
        let payload = r#"{"choices":[{"delta":{"content":"hi"},"finish_reason":null}]}"#;
        let delta = parse_sse_event(payload, ProviderProtocol::ChatCompletions).expect("delta");
        assert_eq!(delta.content, "hi");
    }

    #[test]
    fn parses_responses_sse_delta() {
        let payload = r#"{"type":"response.output_text.delta","delta":"hi"}"#;
        let delta = parse_sse_event(payload, ProviderProtocol::Responses).expect("delta");
        assert_eq!(delta.content, "hi");
    }
}
