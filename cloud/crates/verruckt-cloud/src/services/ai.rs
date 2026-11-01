use crate::config::AiConfig;
use crate::error::ApiError;
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ChatMeta {
    pub logical_model: String,
    pub upstream_model: String,
    pub message_count: usize,
    pub max_tokens: u32,
    pub estimated_input_tokens: i64,
    pub stream: bool,
}

#[derive(Debug, Clone)]
pub struct EmbeddingMeta {
    pub logical_model: String,
    pub upstream_model: String,
    pub input_count: usize,
    pub total_chars: usize,
    pub estimated_input_tokens: i64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Usage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Clone)]
pub struct AiGateway {
    config: AiConfig,
    client: reqwest::Client,
}

impl AiGateway {
    pub fn new(config: AiConfig, timeout: Duration) -> Result<Self, ApiError> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|error| {
                ApiError::internal_message(format!("failed to build AI client: {error}"))
            })?;
        Ok(Self { config, client })
    }

    pub fn config(&self) -> &AiConfig {
        &self.config
    }

    pub fn validate_chat(&self, body: &Value) -> Result<ChatMeta, ApiError> {
        let logical_model = body
            .get("model")
            .and_then(Value::as_str)
            .ok_or_else(|| ApiError::bad_request("invalid_model", "model is required"))?;
        let upstream_model = self
            .config
            .models
            .get(logical_model)
            .cloned()
            .ok_or_else(|| {
                ApiError::bad_request(
                    "unsupported_model",
                    format!("model '{logical_model}' is not supported"),
                )
            })?;

        let messages = body
            .get("messages")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                ApiError::bad_request("invalid_messages", "messages must be an array")
            })?;
        if messages.is_empty() || messages.len() > self.config.max_messages {
            return Err(ApiError::bad_request(
                "invalid_messages",
                format!(
                    "messages must contain between 1 and {} items",
                    self.config.max_messages
                ),
            ));
        }

        let max_tokens = body
            .get("max_tokens")
            .and_then(Value::as_u64)
            .map(|value| value.min(self.config.max_tokens as u64) as u32)
            .unwrap_or(1024)
            .clamp(1, self.config.max_tokens);
        let stream = body.get("stream").and_then(Value::as_bool).unwrap_or(false);

        let mut estimated_input_tokens = 0i64;
        for message in messages {
            estimated_input_tokens += estimate_message_tokens(message);
        }
        estimated_input_tokens = estimated_input_tokens.max(1);

        Ok(ChatMeta {
            logical_model: logical_model.to_string(),
            upstream_model,
            message_count: messages.len(),
            max_tokens,
            estimated_input_tokens,
            stream,
        })
    }

    pub fn validate_embeddings(&self, body: &Value) -> Result<EmbeddingMeta, ApiError> {
        let logical_model = body
            .get("model")
            .and_then(Value::as_str)
            .ok_or_else(|| ApiError::bad_request("invalid_model", "model is required"))?;
        let upstream_model = self
            .config
            .models
            .get(logical_model)
            .cloned()
            .ok_or_else(|| {
                ApiError::bad_request(
                    "unsupported_model",
                    format!("model '{logical_model}' is not supported"),
                )
            })?;

        let input = body
            .get("input")
            .ok_or_else(|| ApiError::bad_request("invalid_input", "input is required"))?;
        let inputs = match input {
            Value::String(value) => vec![value.as_str()],
            Value::Array(values) => {
                let mut result = Vec::with_capacity(values.len());
                for value in values {
                    let Some(value) = value.as_str() else {
                        return Err(ApiError::bad_request(
                            "invalid_input",
                            "embedding input array must contain only strings",
                        ));
                    };
                    result.push(value);
                }
                result
            }
            _ => {
                return Err(ApiError::bad_request(
                    "invalid_input",
                    "embedding input must be a string or an array of strings",
                ));
            }
        };

        if inputs.is_empty() || inputs.len() > self.config.max_embedding_inputs {
            return Err(ApiError::bad_request(
                "invalid_input",
                format!(
                    "embedding input must contain between 1 and {} items",
                    self.config.max_embedding_inputs
                ),
            ));
        }

        let total_chars: usize = inputs.iter().map(|value| value.chars().count()).sum();
        if total_chars > self.config.max_embedding_total_chars {
            return Err(ApiError::bad_request(
                "input_too_large",
                format!(
                    "embedding input exceeds the {} character limit",
                    self.config.max_embedding_total_chars
                ),
            ));
        }

        let estimated_input_tokens = ((total_chars as i64) / 4 + inputs.len() as i64).max(1);
        Ok(EmbeddingMeta {
            logical_model: logical_model.to_string(),
            upstream_model,
            input_count: inputs.len(),
            total_chars,
            estimated_input_tokens,
        })
    }

    pub async fn chat_completion(
        &self,
        upstream_model: &str,
        body: &Value,
    ) -> Result<reqwest::Response, ApiError> {
        let meta = self.validate_chat(body)?;
        let mut object = serde_json::Map::new();
        for key in [
            "messages",
            "temperature",
            "top_p",
            "stop",
            "response_format",
            "seed",
            "reasoning_effort",
        ] {
            if let Some(value) = body.get(key) {
                object.insert(key.into(), value.clone());
            }
        }
        object.insert("max_tokens".into(), serde_json::json!(meta.max_tokens));
        object.insert("n".into(), serde_json::json!(1));
        object.insert("stream".into(), serde_json::json!(meta.stream));
        let mut upstream_body = Value::Object(object);
        if let Value::Object(ref mut object) = upstream_body {
            object.insert("model".into(), Value::String(upstream_model.into()));
            if object
                .get("stream")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                object.insert(
                    "stream_options".into(),
                    serde_json::json!({ "include_usage": true }),
                );
            }
        }
        self.post("/chat/completions", &upstream_body).await
    }

    pub async fn embeddings(
        &self,
        upstream_model: &str,
        body: &Value,
    ) -> Result<reqwest::Response, ApiError> {
        let mut upstream_body = body.clone();
        if let Value::Object(ref mut object) = upstream_body {
            object.insert("model".into(), Value::String(upstream_model.into()));
        }
        self.post("/embeddings", &upstream_body).await
    }

    async fn post(&self, path: &str, body: &Value) -> Result<reqwest::Response, ApiError> {
        let url = format!("{}{}", self.config.base_url.trim_end_matches('/'), path);
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.config.api_key)
            .json(body)
            .send()
            .await
            .map_err(map_reqwest_error)?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::warn!(
                upstream_status = status.as_u16(),
                body = %truncate(body, 256),
                "upstream AI provider returned an error"
            );
            return Err(map_upstream_status(status));
        }
        Ok(response)
    }
}

pub fn extract_usage(value: &Value) -> Usage {
    let usage = value.get("usage").and_then(Value::as_object);
    let input_tokens = usage
        .and_then(|usage| usage.get("prompt_tokens"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let output_tokens = usage
        .and_then(|usage| usage.get("completion_tokens"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let total_tokens = usage
        .and_then(|usage| usage.get("total_tokens"))
        .and_then(Value::as_i64)
        .unwrap_or(input_tokens + output_tokens);
    Usage {
        input_tokens,
        output_tokens,
        total_tokens: total_tokens.max(input_tokens + output_tokens),
    }
}

pub fn estimate_message_tokens(message: &Value) -> i64 {
    match message.get("content") {
        Some(Value::String(value)) => estimate_tokens(value),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|part| {
                part.get("text")
                    .and_then(Value::as_str)
                    .or_else(|| part.as_str())
            })
            .map(estimate_tokens)
            .sum(),
        _ => 1,
    }
}

pub fn estimate_tokens(value: &str) -> i64 {
    ((value.chars().count() as i64) / 4).max(1)
}

pub fn estimate_embedding_tokens(total_chars: usize, count: usize) -> i64 {
    ((total_chars as i64) / 4 + count as i64).max(1)
}

fn map_reqwest_error(error: reqwest::Error) -> ApiError {
    if error.is_timeout() {
        ApiError::gateway_timeout("upstream_timeout", "the upstream AI provider timed out")
    } else {
        tracing::warn!(error = %error, "upstream AI request failed");
        ApiError::bad_gateway(
            "upstream_unavailable",
            "the upstream AI provider is unavailable",
        )
    }
}

fn map_upstream_status(status: reqwest::StatusCode) -> ApiError {
    match status.as_u16() {
        401 | 403 => ApiError::bad_gateway(
            "upstream_auth_failed",
            "the upstream AI provider rejected the server credentials",
        ),
        429 => ApiError::too_many_requests(
            "upstream_rate_limited",
            "the upstream AI provider is rate limiting requests",
        ),
        400..=499 => ApiError::bad_gateway(
            "upstream_rejected_request",
            "the upstream AI provider rejected the request",
        ),
        _ => ApiError::bad_gateway(
            "upstream_error",
            "the upstream AI provider returned an error",
        ),
    }
}

fn truncate(value: String, max: usize) -> String {
    if value.len() <= max {
        return value;
    }
    // Upstream error bodies are frequently UTF-8 (Chinese prompts/messages);
    // byte slicing at an arbitrary offset would panic on a multi-byte char.
    let mut end = max.min(value.len());
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &value[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_openai_usage() {
        let value = serde_json::json!({
            "usage": {"prompt_tokens": 12, "completion_tokens": 7, "total_tokens": 19}
        });
        let usage = extract_usage(&value);
        assert_eq!(usage.input_tokens, 12);
        assert_eq!(usage.output_tokens, 7);
        assert_eq!(usage.total_tokens, 19);
    }

    #[test]
    fn estimates_tokens_from_text() {
        assert!(estimate_tokens("hello world") >= 1);
        assert!(estimate_tokens(&"a".repeat(400)) >= 100);
    }
}
