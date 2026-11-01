use super::dto::{
    AdaptationEventDto, CourseCreateInput, CourseDependencyDto, CourseDetailDto, CourseDto,
    CourseNodeCompleteDto, CourseNodeDto, JobDto, LearnerStateDto, LessonDto,
};
use crate::db::repo::{courses as course_repo, jobs as job_repo, learning_loop as loop_repo};
use crate::error::AppError;
use crate::ipc::State;
use crate::services::course_service::{CourseService, RouteRequest};
use crate::state::AppState;
use serde_json::json;
use uuid::Uuid;

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn course_list(state: State<'_, AppState>) -> Result<Vec<CourseDto>, AppError> {
    let courses = CourseService::new(&state).list_courses().await?;
    let mut output = Vec::with_capacity(courses.len());
    for course in &courses {
        let progress = course_repo::progress_for_course(&state.pool, course.id).await?;
        output.push(CourseDto::from_course(course, &progress));
    }
    Ok(output)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn course_get(state: State<'_, AppState>, id: Uuid) -> Result<CourseDetailDto, AppError> {
    let (course, nodes, dependencies) = CourseService::new(&state).get_course_bundle(id).await?;
    let progress = course_repo::progress_for_course(&state.pool, id).await?;
    Ok(CourseDetailDto {
        course: CourseDto::from_course(&course, &progress),
        nodes: nodes.iter().map(CourseNodeDto::from).collect(),
        dependencies: dependencies.iter().map(CourseDependencyDto::from).collect(),
    })
}

/// Start roadmap generation as a real background job.
///
/// Progress phases are published through `job://progress`; the created route id
/// lands in the job's `resultJson` so the UI can open it when the job finishes.
#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn course_create(
    state: State<'_, AppState>,
    input: CourseCreateInput,
) -> Result<JobDto, AppError> {
    let request = RouteRequest {
        topic: input.topic,
        goal: input.goal,
        experience_level: input.experience_level,
        desired_depth: input.desired_depth,
        source_ids: input.source_ids,
    };
    let state = state.inner().clone();
    let jobs = state.jobs.clone();
    let pool = state.pool.clone();
    let job = jobs
        .start("generate_route", move |handle| async move {
            let course = CourseService::new(&state)
                .create_course(request, Some(&handle))
                .await?;
            job_repo::set_job_result(
                &pool,
                handle.id(),
                &json!({ "courseId": course.id }).to_string(),
            )
            .await?;
            handle.progress(1.0, "Ready").await?;
            Ok(())
        })
        .await?;
    Ok(JobDto::from(&job))
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn course_regenerate(state: State<'_, AppState>, id: Uuid) -> Result<JobDto, AppError> {
    course_repo::get_course(&state.pool, id).await?;
    let state = state.inner().clone();
    let jobs = state.jobs.clone();
    let pool = state.pool.clone();
    let job = jobs
        .start("regenerate_route", move |handle| async move {
            let course = CourseService::new(&state)
                .regenerate_course(id, Some(&handle))
                .await?;
            job_repo::set_job_result(
                &pool,
                handle.id(),
                &json!({ "courseId": course.id, "replacedCourseId": id }).to_string(),
            )
            .await?;
            handle.progress(1.0, "Ready").await?;
            Ok(())
        })
        .await?;
    Ok(JobDto::from(&job))
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn course_delete(state: State<'_, AppState>, id: Uuid) -> Result<(), AppError> {
    CourseService::new(&state).delete_course(id).await
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn course_node_get(
    state: State<'_, AppState>,
    node_id: Uuid,
) -> Result<CourseNodeDto, AppError> {
    let node = course_repo::get_node(&state.pool, node_id).await?;
    Ok(CourseNodeDto::from(&node))
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn course_node_prepare(
    state: State<'_, AppState>,
    node_id: Uuid,
) -> Result<LessonDto, AppError> {
    let prepared = CourseService::new(&state)
        .prepare_node(node_id, None)
        .await?;
    let node = course_repo::get_node(&state.pool, node_id).await?;
    Ok(LessonDto::from_prepared(&prepared, &node))
}

/// Close a node: mark it complete, unlock the next one and record the
/// adaptation event. Returns the refreshed course so the roadmap can re-render
/// without a second round-trip.
#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn course_node_complete(
    state: State<'_, AppState>,
    node_id: Uuid,
) -> Result<CourseNodeCompleteDto, AppError> {
    let course = CourseService::new(&state).complete_node(node_id).await?;
    let progress = course_repo::progress_for_course(&state.pool, course.id).await?;
    let events = loop_repo::list_adaptations(&state.pool, course.id, 1).await?;
    Ok(CourseNodeCompleteDto {
        course: CourseDto::from_course(&course, &progress),
        completed_node_id: node_id,
        next_node_id: progress.current_node_id,
        adaptation: events.first().map(AdaptationEventDto::from),
    })
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn course_learner_states(
    state: State<'_, AppState>,
    id: Uuid,
) -> Result<Vec<LearnerStateDto>, AppError> {
    let states = loop_repo::list_learner_states(&state.pool, id).await?;
    let points = loop_repo::list_knowledge_points(&state.pool, id).await?;
    let now = crate::util::time::utc_now();
    Ok(states
        .iter()
        .map(|state| {
            let title = points
                .iter()
                .find(|point| point.id == state.knowledge_point_id)
                .map(|point| point.title.clone())
                .unwrap_or_else(|| "未命名知识点".to_owned());
            LearnerStateDto {
                knowledge_point_id: state.knowledge_point_id,
                title,
                mastery: state.mastery,
                confidence: state.confidence,
                mistakes: state.mistakes,
                attempts: state.attempts,
                last_seen: state.last_seen.map(|value| value.to_rfc3339()),
                next_review: state.next_review.map(|value| value.to_rfc3339()),
                review_due: state.is_review_due(now),
            }
        })
        .collect())
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn course_adaptations(
    state: State<'_, AppState>,
    id: Uuid,
    limit: Option<i64>,
) -> Result<Vec<AdaptationEventDto>, AppError> {
    let events =
        loop_repo::list_adaptations(&state.pool, id, limit.unwrap_or(20).clamp(1, 200)).await?;
    Ok(events.iter().map(AdaptationEventDto::from).collect())
}
