use super::dto::{
    CourseCreateInput, CourseDependencyDto, CourseDetailDto, CourseDto, CourseNodeDto, JobDto,
    LessonDto,
};
use crate::db::repo::courses as course_repo;
use crate::error::AppError;
use crate::services::course_service::CourseService;
use crate::state::AppState;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub async fn course_list(state: State<'_, AppState>) -> Result<Vec<CourseDto>, AppError> {
    let courses = CourseService::new(&state).list_courses().await?;
    Ok(courses.iter().map(CourseDto::from).collect())
}

#[tauri::command]
pub async fn course_get(state: State<'_, AppState>, id: Uuid) -> Result<CourseDetailDto, AppError> {
    let (course, nodes, dependencies) = CourseService::new(&state).get_course_bundle(id).await?;
    Ok(CourseDetailDto {
        course: CourseDto::from(&course),
        nodes: nodes.iter().map(CourseNodeDto::from).collect(),
        dependencies: dependencies.iter().map(CourseDependencyDto::from).collect(),
    })
}

#[tauri::command]
pub async fn course_create(
    state: State<'_, AppState>,
    input: CourseCreateInput,
) -> Result<JobDto, AppError> {
    let state = state.inner().clone();
    let jobs = state.jobs.clone();
    let job = jobs
        .start("course_generation", move |handle| async move {
            CourseService::new(&state)
                .create_course(&input.topic, Some(&handle))
                .await
                .map(|_| ())
        })
        .await?;
    Ok(JobDto::from(&job))
}

#[tauri::command]
pub async fn course_regenerate(state: State<'_, AppState>, id: Uuid) -> Result<JobDto, AppError> {
    course_repo::get_course(&state.pool, id).await?;
    let state = state.inner().clone();
    let jobs = state.jobs.clone();
    let job = jobs
        .start("course_regeneration", move |handle| async move {
            CourseService::new(&state)
                .regenerate_course(id, Some(&handle))
                .await
                .map(|_| ())
        })
        .await?;
    Ok(JobDto::from(&job))
}

#[tauri::command]
pub async fn course_delete(state: State<'_, AppState>, id: Uuid) -> Result<(), AppError> {
    CourseService::new(&state).delete_course(id).await
}

#[tauri::command]
pub async fn course_node_get(
    state: State<'_, AppState>,
    node_id: Uuid,
) -> Result<CourseNodeDto, AppError> {
    let node = course_repo::get_node(&state.pool, node_id).await?;
    Ok(CourseNodeDto::from(&node))
}

#[tauri::command]
pub async fn course_node_prepare(
    state: State<'_, AppState>,
    node_id: Uuid,
) -> Result<LessonDto, AppError> {
    let prepared = CourseService::new(&state)
        .prepare_node(node_id, None)
        .await?;
    Ok(LessonDto::from_lesson(
        &prepared.lesson,
        &prepared.citations,
    ))
}
