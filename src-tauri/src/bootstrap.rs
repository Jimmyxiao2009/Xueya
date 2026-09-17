use crate::error::AppError;
use crate::state::{AppPaths, AppState};
use tauri::Manager;

pub async fn build_state(app: &tauri::AppHandle) -> Result<AppState, AppError> {
    let data_dir = app.path().app_data_dir().map_err(|error| {
        AppError::Internal(format!("could not resolve app data directory: {error}"))
    })?;
    AppState::new(AppPaths::new(data_dir)).await
}
