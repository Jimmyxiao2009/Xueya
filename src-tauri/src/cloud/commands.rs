use super::models::{CloudAccountState, CloudDeviceDto, CloudMeDto, CloudUsageDto};
use crate::error::AppError;
use crate::state::AppState;
use serde::Deserialize;
use tauri::State;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudCredentialsInput {
    pub email: String,
    pub password: String,
}

#[tauri::command]
pub async fn cloud_account_state(
    state: State<'_, AppState>,
) -> Result<CloudAccountState, AppError> {
    state.cloud.account_state().await
}

#[tauri::command]
pub async fn cloud_register(
    state: State<'_, AppState>,
    input: CloudCredentialsInput,
) -> Result<CloudAccountState, AppError> {
    state.cloud.register(input.email, input.password).await
}

#[tauri::command]
pub async fn cloud_login(
    state: State<'_, AppState>,
    input: CloudCredentialsInput,
) -> Result<CloudAccountState, AppError> {
    state.cloud.login(input.email, input.password).await
}

#[tauri::command]
pub async fn cloud_logout(state: State<'_, AppState>) -> Result<CloudAccountState, AppError> {
    state.cloud.logout().await
}

#[tauri::command]
pub async fn cloud_get_me(state: State<'_, AppState>) -> Result<CloudMeDto, AppError> {
    let me = state.cloud.me().await?;
    Ok(me.user.into())
}

#[tauri::command]
pub async fn cloud_get_entitlements(
    state: State<'_, AppState>,
) -> Result<super::models::CloudEntitlementsDto, AppError> {
    let entitlements = state.cloud.entitlements().await?;
    Ok((&entitlements).into())
}

#[tauri::command]
pub async fn cloud_list_devices(
    state: State<'_, AppState>,
) -> Result<Vec<CloudDeviceDto>, AppError> {
    Ok(state
        .cloud
        .devices()
        .await?
        .into_iter()
        .map(CloudDeviceDto::from)
        .collect())
}

#[tauri::command]
pub async fn cloud_remove_device(state: State<'_, AppState>, id: Uuid) -> Result<(), AppError> {
    state.cloud.remove_device(id).await
}

#[tauri::command]
pub async fn cloud_get_ai_usage(state: State<'_, AppState>) -> Result<CloudUsageDto, AppError> {
    let usage = state.cloud.ai_usage().await?;
    Ok((&usage).into())
}

#[tauri::command]
pub async fn cloud_set_base_url(
    state: State<'_, AppState>,
    url: String,
) -> Result<CloudAccountState, AppError> {
    state.cloud.set_base_url(&url).await
}

#[tauri::command]
pub async fn cloud_set_sync_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<CloudAccountState, AppError> {
    state.cloud.set_sync_enabled(enabled).await
}
