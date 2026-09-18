use crate::auth::AuthenticatedUser;
use crate::error::ApiError;
use crate::request_id::current_request_id;
use crate::services::ai::{estimate_tokens, extract_usage, AiGateway, Usage};
use crate::services::entitlements::resolve_entitlements;
use crate::services::quota::{
    commit_ai_usage, get_ai_usage, release_ai_reservation, reserve_ai_credits, AiUsageEventInput,
};
use crate::state::AppState;
use async_stream::stream;
use axum::body::Body;
use axum::extract::State;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bytes::Bytes;
use futures_util::StreamExt;
use serde_json::Value;
use std::convert::Infallible;
use std::time::Instant;
use uuid::Uuid;
use verruckt_cloud_protocol::ai::AiUsageResponse;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/ai/chat", post(chat))
        .route("/api/v1/ai/chat/stream", post(chat_stream))
        .route("/api/v1/ai/embeddings", post(embeddings))
        .route("/api/v1/ai/usage", get(usage))
        .route("/v1/chat/completions", post(openai_chat_completions))
        .route("/v1/embeddings", post(embeddings))
}

async fn chat(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    Json(body): Json<Value>,
) -> Result<Response, ApiError> {
    run_chat(state, authenticated.user_id, body, Some(false)).await
}

async fn chat_stream(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    Json(body): Json<Value>,
) -> Result<Response, ApiError> {
    run_chat(state, authenticated.user_id, body, Some(true)).await
}

async fn openai_chat_completions(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    Json(body): Json<Value>,
) -> Result<Response, ApiError> {
    run_chat(state, authenticated.user_id, body, None).await
}

async fn embeddings(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    Json(body): Json<Value>,
) -> Result<Response, ApiError> {
    run_embeddings(state, authenticated.user_id, body).await
}

async fn usage(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
) -> Result<Json<AiUsageResponse>, ApiError> {
    Ok(Json(get_ai_usage(&state, authenticated.user_id).await?))
}

async fn run_chat(
    state: AppState,
    user_id: Uuid,
    mut body: Value,
    force_stream: Option<bool>,
) -> Result<Response, ApiError> {
    let ai = require_ai(&state)?;
    enforce_ai_entitlement(&state, user_id).await?;
    check_rate_limit(&state, user_id)?;

    if let Some(stream) = force_stream {
        if let Value::Object(ref mut object) = body {
            object.insert("stream".into(), Value::Bool(stream));
        }
    }

    let meta = ai.validate_chat(&body)?;
    let limit = resolve_entitlements(&state.pool, user_id)
        .await?
        .monthly_ai_credits();
    let estimated_units = meta.estimated_input_tokens + (meta.max_tokens as i64).min(4096);
    let reservation_id = reserve_ai_credits(
        &state,
        user_id,
        &meta.logical_model,
        estimated_units,
        limit,
        120,
    )
    .await?;

    let started = Instant::now();
    let upstream = ai.chat_completion(&meta.upstream_model, &body).await;
    let response = match upstream {
        Ok(response) => {
            state.provider_status.mark_ok();
            response
        }
        Err(error) => {
            state.provider_status.mark_degraded();
            let _ = release_ai_reservation(&state, user_id, reservation_id).await;
            return Err(error);
        }
    };

    if meta.stream {
        Ok(stream_chat_response(
            state,
            user_id,
            reservation_id,
            meta,
            started,
            response,
        ))
    } else {
        let value: Value = response.json().await.map_err(|error| {
            tracing::warn!(error = %error, "failed to decode upstream chat response");
            ApiError::bad_gateway(
                "invalid_upstream_response",
                "the upstream AI provider returned an invalid response",
            )
        })?;

        let usage = extract_usage(&value);
        let actual_units = if usage.total_tokens > 0 {
            usage.total_tokens
        } else {
            meta.estimated_input_tokens + estimate_completion_tokens(&value)
        };
        let request_id = current_request_id();
        commit_ai_usage(
            &state,
            user_id,
            reservation_id,
            actual_units,
            AiUsageEventInput {
                operation: "chat".into(),
                logical_model: meta.logical_model.clone(),
                upstream_model: meta.upstream_model.clone(),
                input_tokens: usage.input_tokens.max(meta.estimated_input_tokens),
                output_tokens: usage.output_tokens.max(estimate_completion_tokens(&value)),
                estimated_cost_units: actual_units,
                status: "completed".into(),
                request_id,
                latency_ms: started.elapsed().as_millis() as i64,
            },
        )
        .await?;

        let mut response_value = value;
        if let Value::Object(ref mut object) = response_value {
            object.insert("model".into(), Value::String(meta.logical_model.clone()));
        }
        Ok(Json(response_value).into_response())
    }
}

async fn run_embeddings(state: AppState, user_id: Uuid, body: Value) -> Result<Response, ApiError> {
    let ai = require_ai(&state)?;
    enforce_ai_entitlement(&state, user_id).await?;
    check_rate_limit(&state, user_id)?;

    let meta = ai.validate_embeddings(&body)?;
    let limit = resolve_entitlements(&state.pool, user_id)
        .await?
        .monthly_ai_credits();
    let reservation_id = reserve_ai_credits(
        &state,
        user_id,
        &meta.logical_model,
        meta.estimated_input_tokens,
        limit,
        120,
    )
    .await?;

    let started = Instant::now();
    let upstream = ai.embeddings(&meta.upstream_model, &body).await;
    let response = match upstream {
        Ok(response) => {
            state.provider_status.mark_ok();
            response
        }
        Err(error) => {
            state.provider_status.mark_degraded();
            let _ = release_ai_reservation(&state, user_id, reservation_id).await;
            return Err(error);
        }
    };

    let value: Value = response.json().await.map_err(|error| {
        tracing::warn!(error = %error, "failed to decode upstream embedding response");
        ApiError::bad_gateway(
            "invalid_upstream_response",
            "the upstream AI provider returned an invalid response",
        )
    })?;

    let usage = extract_usage(&value);
    let actual_units = if usage.total_tokens > 0 {
        usage.total_tokens
    } else {
        meta.estimated_input_tokens
    };
    commit_ai_usage(
        &state,
        user_id,
        reservation_id,
        actual_units,
        AiUsageEventInput {
            operation: "embeddings".into(),
            logical_model: meta.logical_model.clone(),
            upstream_model: meta.upstream_model.clone(),
            input_tokens: usage.input_tokens.max(meta.estimated_input_tokens),
            output_tokens: 0,
            estimated_cost_units: actual_units,
            status: "completed".into(),
            request_id: current_request_id(),
            latency_ms: started.elapsed().as_millis() as i64,
        },
    )
    .await?;

    let mut response_value = value;
    if let Value::Object(ref mut object) = response_value {
        object.insert("model".into(), Value::String(meta.logical_model.clone()));
    }
    Ok(Json(response_value).into_response())
}

fn stream_chat_response(
    state: AppState,
    user_id: Uuid,
    reservation_id: Uuid,
    meta: crate::services::ai::ChatMeta,
    started: Instant,
    response: reqwest::Response,
) -> Response {
    let request_id = current_request_id();
    let stream = stream! {
        let mut guard = ReservationGuard::new(state.clone(), user_id, reservation_id);
        let mut upstream = response.bytes_stream();
        let mut sse_buffer: Vec<u8> = Vec::new();
        let mut usage: Option<Usage> = None;
        let mut output_chars = 0usize;
        let mut upstream_failed = false;

        while let Some(item) = upstream.next().await {
            match item {
                Ok(chunk) => {
                    sse_buffer.extend_from_slice(&chunk);
                    while let Some(position) = sse_buffer.iter().position(|byte| *byte == b'\n') {
                        let line = sse_buffer.drain(..=position).collect::<Vec<u8>>();
                        let line = String::from_utf8_lossy(&line);
                        if let Some(data) = line.strip_prefix("data: ") {
                            let data = data.trim();
                            if data == "[DONE]" {
                                continue;
                            }
                            if let Ok(value) = serde_json::from_str::<Value>(data) {
                                if let Some(found) = parse_stream_usage(&value) {
                                    usage = Some(found);
                                }
                                if let Some(content) = value
                                    .pointer("/choices/0/delta/content")
                                    .and_then(Value::as_str)
                                {
                                    output_chars += content.chars().count();
                                }
                            }
                        }
                    }
                    yield Ok::<Bytes, Infallible>(chunk);
                }
                Err(error) => {
                    tracing::warn!(error = %error, "upstream SSE stream failed");
                    upstream_failed = true;
                    break;
                }
            }
        }

        let actual_units = usage
            .map(|usage| usage.total_tokens.max(usage.input_tokens + usage.output_tokens))
            .filter(|units| *units > 0)
            .unwrap_or_else(|| {
                meta.estimated_input_tokens + estimate_tokens(&"x".repeat(output_chars)).max(1)
            });
        let output_tokens = usage
            .map(|usage| usage.output_tokens)
            .unwrap_or_else(|| estimate_tokens(&"x".repeat(output_chars)).max(1));
        let event = AiUsageEventInput {
            operation: "chat_stream".into(),
            logical_model: meta.logical_model.clone(),
            upstream_model: meta.upstream_model.clone(),
            input_tokens: usage.map(|usage| usage.input_tokens).unwrap_or(meta.estimated_input_tokens),
            output_tokens,
            estimated_cost_units: actual_units,
            status: if upstream_failed { "upstream_error".into() } else { "completed".into() },
            request_id,
            latency_ms: started.elapsed().as_millis() as i64,
        };
        match commit_ai_usage(&state, user_id, reservation_id, actual_units, event).await {
            Ok(()) => guard.disarm(),
            Err(error) => {
                tracing::error!(error = ?error, "failed to commit stream usage; reservation will be released");
            }
        }
    };

    let mut response = Body::from_stream(stream).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("text/event-stream"),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-cache"),
    );
    response
}

fn parse_stream_usage(value: &Value) -> Option<Usage> {
    value
        .get("usage")
        .map(|_| extract_usage(value))
        .filter(|usage| usage.input_tokens > 0 || usage.output_tokens > 0 || usage.total_tokens > 0)
}

fn estimate_completion_tokens(value: &Value) -> i64 {
    value
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(estimate_tokens)
        .unwrap_or(1)
}

fn require_ai(state: &AppState) -> Result<&AiGateway, ApiError> {
    state.ai.as_deref().ok_or_else(|| {
        ApiError::service_unavailable(
            "ai_not_configured",
            "the official AI gateway is not configured",
        )
    })
}

async fn enforce_ai_entitlement(state: &AppState, user_id: Uuid) -> Result<(), ApiError> {
    let entitlements = resolve_entitlements(&state.pool, user_id).await?;
    if !entitlements.official_ai_enabled() {
        return Err(ApiError::forbidden(
            "entitlement_required",
            "official_ai entitlement is required",
        ));
    }
    Ok(())
}

fn check_rate_limit(state: &AppState, user_id: Uuid) -> Result<(), ApiError> {
    state.rate_limiter.check(user_id).map_err(|retry_after| {
        ApiError::too_many_requests(
            "rate_limited",
            format!("too many AI requests; retry after {retry_after} seconds"),
        )
    })
}

struct ReservationGuard {
    state: AppState,
    user_id: Uuid,
    reservation_id: Uuid,
    armed: bool,
}

impl ReservationGuard {
    fn new(state: AppState, user_id: Uuid, reservation_id: Uuid) -> Self {
        Self {
            state,
            user_id,
            reservation_id,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ReservationGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let state = self.state.clone();
        let user_id = self.user_id;
        let reservation_id = self.reservation_id;
        tokio::spawn(async move {
            let _ = release_ai_reservation(&state, user_id, reservation_id).await;
        });
    }
}
