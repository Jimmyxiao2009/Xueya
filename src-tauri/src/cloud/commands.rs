use super::models::{CloudAccountState, CloudDeviceDto, CloudMeDto, CloudUsageDto};
use crate::error::AppError;
use crate::ipc::State;
use crate::services::settings_service;
use crate::state::AppState;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudCredentialsInput {
    pub email: String,
    pub password: String,
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn cloud_account_state(
    state: State<'_, AppState>,
) -> Result<CloudAccountState, AppError> {
    state.cloud.account_state().await
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn cloud_register(
    state: State<'_, AppState>,
    input: CloudCredentialsInput,
) -> Result<CloudAccountState, AppError> {
    let account = state.cloud.register(input.email, input.password).await?;
    settings_service::reconcile_provider_policy(&state, &account).await?;
    Ok(account)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn cloud_login(
    state: State<'_, AppState>,
    input: CloudCredentialsInput,
) -> Result<CloudAccountState, AppError> {
    let account = state.cloud.login(input.email, input.password).await?;
    settings_service::reconcile_provider_policy(&state, &account).await?;
    Ok(account)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn cloud_logout(state: State<'_, AppState>) -> Result<CloudAccountState, AppError> {
    let account = state.cloud.logout().await?;
    settings_service::reconcile_provider_policy(&state, &account).await?;
    Ok(account)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn cloud_get_me(state: State<'_, AppState>) -> Result<CloudMeDto, AppError> {
    let me = state.cloud.me().await?;
    Ok(me.user.into())
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn cloud_get_entitlements(
    state: State<'_, AppState>,
) -> Result<super::models::CloudEntitlementsDto, AppError> {
    let entitlements = state.cloud.entitlements().await?;
    Ok((&entitlements).into())
}

#[cfg_attr(feature = "desktop", tauri::command)]
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

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn cloud_remove_device(state: State<'_, AppState>, id: Uuid) -> Result<(), AppError> {
    state.cloud.remove_device(id).await
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn cloud_get_ai_usage(state: State<'_, AppState>) -> Result<CloudUsageDto, AppError> {
    let usage = state.cloud.ai_usage().await?;
    Ok((&usage).into())
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn cloud_set_base_url(
    state: State<'_, AppState>,
    url: String,
) -> Result<CloudAccountState, AppError> {
    state.cloud.set_base_url(&url).await
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn cloud_set_sync_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<CloudAccountState, AppError> {
    state.cloud.set_sync_enabled(enabled).await
}
