use super::dto::{ProviderDto, ProviderTestResultDto, ProviderUpsertInput};
use crate::ai::registry::ProviderUpsert;
use crate::error::AppError;
use crate::state::AppState;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub async fn provider_list(state: State<'_, AppState>) -> Result<Vec<ProviderDto>, AppError> {
    let providers = state.providers.list().await?;
    let mut output = Vec::with_capacity(providers.len());
    for provider in providers {
        let has_api_key = state.providers.has_api_key(&provider).await?;
        output.push(ProviderDto::from_config(provider, has_api_key).await);
    }
    Ok(output)
}

#[tauri::command]
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
            enabled: input.enabled.unwrap_or(true),
        })
        .await?;
    let has_api_key = state.providers.has_api_key(&provider).await?;
    Ok(ProviderDto::from_config(provider, has_api_key).await)
}

#[tauri::command]
pub async fn provider_delete(state: State<'_, AppState>, id: Uuid) -> Result<(), AppError> {
    state.providers.delete(id).await
}

#[tauri::command]
pub async fn provider_test(
    state: State<'_, AppState>,
    id: Uuid,
) -> Result<ProviderTestResultDto, AppError> {
    let provider = state.providers.get(id).await?;
    let chat = state.providers.chat_provider(&provider).await?;
    if let Err(error) = chat.test_connection().await {
        return Ok(ProviderTestResultDto {
            ok: false,
            message: error.public_message().to_owned(),
        });
    }
    Ok(ProviderTestResultDto {
        ok: true,
        message: "Connection successful".into(),
    })
}
