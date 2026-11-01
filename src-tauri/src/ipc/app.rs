use super::dto::AppInfoDto;
use crate::error::AppError;
use crate::ipc::State;
use crate::services::{course_service::CourseService, settings_service, source_service};
use crate::state::AppState;

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn app_bootstrap(state: State<'_, AppState>) -> Result<AppInfoDto, AppError> {
    settings_service::initialize(&state).await?;
    app_info(&state).await
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn app_get_info(state: State<'_, AppState>) -> Result<AppInfoDto, AppError> {
    app_info(&state).await
}

async fn app_info(state: &AppState) -> Result<AppInfoDto, AppError> {
    let provider_count = state
        .providers
        .list()
        .await?
        .iter()
        .filter(|provider| provider.id != crate::ai::registry::MOCK_PROVIDER_ID)
        .count();
    let source_count = source_service::list_sources(&state.pool).await?.len();
    let course_count = CourseService::new(state).list_courses().await?.len();
    Ok(AppInfoDto {
        version: env!("CARGO_PKG_VERSION").into(),
        data_dir: state.app_data_dir().display().to_string(),
        provider_count,
        source_count,
        course_count,
        search_configured: state.search.is_configured(),
    })
}
