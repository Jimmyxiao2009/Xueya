use crate::db::repo::courses as course_repo;
use crate::error::AppError;
use crate::services::course_service::{CourseService, RouteRequest};
use crate::state::{AppPaths, AppState};
use tauri::Manager;

pub async fn build_state(app: &tauri::AppHandle) -> Result<AppState, AppError> {
    let data_dir = app.path().app_data_dir().map_err(|error| {
        AppError::Internal(format!("could not resolve app data directory: {error}"))
    })?;
    AppState::new(AppPaths::new(data_dir)).await
}

/// First launch ships the two routes from the design prototype so the app is
/// never an empty shell.
///
/// Both are created through the *same* code path as a user-created route — the
/// deterministic curriculum template, with no sources and no API key — and then
/// flagged \`seeded\` so they can be told apart from real learner content.
pub async fn seed_demo_routes(state: &AppState) -> Result<(), AppError> {
    if course_repo::count_courses(&state.pool).await? > 0 {
        return Ok(());
    }
    for topic in ["射影几何", "线性代数"] {
        let service = CourseService::new(state);
        let course = service
            .create_course(RouteRequest::new(topic), None)
            .await?;
        course_repo::mark_seeded(&state.pool, course.id).await?;

        // The prototype opens on a route whose first node is already behind you,
        // so the roadmap shows its completed state on first launch.
        let nodes = course_repo::list_nodes(&state.pool, course.id).await?;
        if let Some(first) = nodes.first() {
            service.complete_node(first.id).await?;
        }
    }
    Ok(())
}
