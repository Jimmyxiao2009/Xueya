use super::dto::{SettingsDto, SettingsUpdateInput};
use crate::error::AppError;
use crate::services::settings_service;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> Result<SettingsDto, AppError> {
    settings_service::get_settings(&state).await
}

#[tauri::command]
pub async fn settings_update(
    state: State<'_, AppState>,
    input: SettingsUpdateInput,
) -> Result<SettingsDto, AppError> {
    settings_service::update_settings(&state, input).await
}
