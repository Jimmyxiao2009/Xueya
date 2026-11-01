use super::dto::{
    ProviderCapabilitiesDto, ProviderDto, ProviderTestResultDto, ProviderUpsertInput,
    StreamChatInput,
};
use crate::ai::registry::{ProviderUpsert, MOCK_PROVIDER_ID};
use crate::ai::traits::{ChatMessage, ChatRequest};
use crate::domain::ProviderProtocol;
use crate::error::AppError;
use crate::ipc::{AppHandle, State};
use crate::state::AppState;
use futures_util::StreamExt;
#[cfg(feature = "desktop")]
use tauri::Emitter;
use uuid::Uuid;

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn provider_list(state: State<'_, AppState>) -> Result<Vec<ProviderDto>, AppError> {
    let providers = state.providers.list().await?;
    let mut output = Vec::with_capacity(providers.len());
    for provider in providers {
        // The offline fallback is an implementation detail, not a user-managed
        // BYOK provider. It is hidden from the settings list.
        if provider.id == MOCK_PROVIDER_ID {
            continue;
        }
        let has_api_key = state.providers.has_api_key(&provider).await?;
        let masked_hint = state.providers.api_key_hint(&provider).await?;
        output.push(ProviderDto::from_config(provider, has_api_key, masked_hint).await);
    }
    Ok(output)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn provider_upsert(
    state: State<'_, AppState>,
    input: ProviderUpsertInput,
) -> Result<ProviderDto, AppError> {
    let provider = state
        .providers
        .upsert(ProviderUpsert {
            id: input.id,
            display_name: input.display_name,
            base_url: input.base_url,
            chat_model: input.chat_model,
            embedding_model: input.embedding_model,
            api_key: input.api_key,
            protocol: ProviderProtocol::parse(input.protocol.as_deref().unwrap_or("auto")),
            enabled: input.enabled.unwrap_or(true),
        })
        .await?;
    let has_api_key = state.providers.has_api_key(&provider).await?;
    let masked_hint = state.providers.api_key_hint(&provider).await?;
    Ok(ProviderDto::from_config(provider, has_api_key, masked_hint).await)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn provider_delete(state: State<'_, AppState>, id: Uuid) -> Result<(), AppError> {
    state.providers.delete(id).await
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn provider_test(
    state: State<'_, AppState>,
    id: Uuid,
) -> Result<ProviderTestResultDto, AppError> {
    let capabilities = state.providers.probe_and_cache(id).await?;
    let ok = capabilities.supports_chat();
    let message = if ok {
        capabilities.status_lines().join("\n")
    } else {
        capabilities
            .probe_errors
            .first()
            .cloned()
            .unwrap_or_else(|| "Provider has no usable chat endpoint".to_owned())
    };
    Ok(ProviderTestResultDto {
        ok,
        message,
        protocol: capabilities
            .protocol
            .map(|protocol| protocol.as_str().to_owned()),
        capabilities: Some(ProviderCapabilitiesDto::from(capabilities)),
    })
}

/// Real provider streaming for the desktop shell.
///
/// Deltas are emitted on the `provider_stream` event as `{ requestId, event }`
/// while the command returns the concatenated final text. The frontend can
/// render incrementally and still receive one authoritative final value.
#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn provider_stream_chat(
    app: AppHandle,
    state: State<'_, AppState>,
    input: StreamChatInput,
) -> Result<String, AppError> {
    let prompt = input.prompt.trim();
    if prompt.is_empty() {
        return Err(AppError::InvalidInput("prompt cannot be empty".into()));
    }
    let provider = state.providers.get(input.provider_id).await?;
    let chat = state.providers.chat_provider(&provider).await?;
    let mut stream = chat
        .chat_stream(ChatRequest::new(
            provider.chat_model.clone(),
            vec![ChatMessage::user(prompt)],
        ))
        .await?;

    let request_id = input.request_id.clone().unwrap_or_default();
    let mut output = String::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(delta) => {
                output.push_str(&delta.content);
                let _ = app.emit(
                    "provider_stream",
                    serde_json::json!({
                        "requestId": request_id,
                        "event": { "type": "delta", "text": delta.content },
                    }),
                );
            }
            Err(error) => {
                let _ = app.emit(
                    "provider_stream",
                    serde_json::json!({
                        "requestId": request_id,
                        "event": {
                            "type": "failed",
                            "code": error.code(),
                            "message": error.public_message(),
                        },
                    }),
                );
                return Err(error);
            }
        }
    }
    let _ = app.emit(
        "provider_stream",
        serde_json::json!({
            "requestId": request_id,
            "event": { "type": "completed", "finishReason": "stop" },
        }),
    );
    Ok(output)
}
