use crate::db::repo::courses as course_repo;
use crate::domain::CourseNodeStatus;
use crate::error::AppError;
use crate::ipc::State;
use crate::services::course_service::{CourseService, RouteRequest};
use crate::services::ingestion_service::{IngestInput, IngestionService};
use crate::state::AppState;
use serde_json::json;
use uuid::Uuid;

const DEMO_TOPIC: &str = "射影几何";
const DEMO_MARKER: &str = "competitionDemo";

const PROJECTIVE_PLANE: &str = r#"# 射影平面与无穷远点

在欧氏平面里，两条平行直线没有交点。把平面补上“无穷远点”之后，每一组平行直线都交于同一个无穷远点。
补上无穷远点与无穷远直线后得到的结构称为射影平面。

- 无穷远点是方向的抽象：同一方向的平行线共享一个无穷远点。
- 无穷远直线是所有无穷远点组成的集合。
- 射影平面上的直线与直线总是相交于一点，平行不再是例外。

这套结构让我们可以统一处理“相交”和“平行”两类情形。
"#;

const HOMOGENEOUS_COORDINATES: &str = r#"# 齐次坐标

点 (x, y) 可以写成三元组 (x : y : 1)。第三个坐标不为 0 时，除以它就能还原欧氏坐标。
当第三个坐标为 0 时，表示无穷远点。

- (x : y : 1) 对应普通点。
- (x : y : 0) 对应方向 (x, y) 上的无穷远点。
- 三元组整体乘以非零常数表示同一个射影点。

齐次坐标把无穷远点纳入同一套代数语言，是射影几何计算的基础。
"#;

/// Seed the deterministic competition demo route.
///
/// This is intentionally unavailable in production: it only runs in debug
/// builds or when `VERRUCKT_COMPETITION_DEMO=1` is explicitly set. It deletes
/// only the route previously marked with the `competitionDemo` metadata flag,
/// never ordinary learner routes. Sources are content-addressed and therefore
/// reused rather than duplicated.
#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn competition_demo_seed(state: State<'_, AppState>) -> Result<Uuid, AppError> {
    seed_competition_demo(&state).await
}

/// Core seeding routine, kept out of the Tauri command wrapper so tests can
/// exercise the same deterministic fixture path without constructing `State`.
pub async fn seed_competition_demo(state: &AppState) -> Result<Uuid, AppError> {
    if !(cfg!(debug_assertions) || std::env::var("VERRUCKT_COMPETITION_DEMO").as_deref() == Ok("1"))
    {
        return Err(AppError::InvalidInput(
            "competition demo is disabled in this build".into(),
        ));
    }

    let courses = course_repo::list_courses(&state.pool).await?;
    for course in courses.into_iter().filter(|course| {
        course.topic == DEMO_TOPIC
            && serde_json::from_str::<serde_json::Value>(&course.metadata_json)
                .ok()
                .and_then(|value| value.get(DEMO_MARKER).and_then(|v| v.as_bool()))
                == Some(true)
    }) {
        CourseService::new(state).delete_course(course.id).await?;
    }

    let plane = IngestionService::new(state)
        .ingest_bytes(
            IngestInput {
                filename: "projective-plane.md".to_owned(),
                bytes: PROJECTIVE_PLANE.as_bytes().to_vec(),
                kind: crate::domain::SourceKind::File,
                title: Some("射影平面与无穷远点".to_owned()),
                uri: None,
                mime_type: Some("text/markdown".to_owned()),
            },
            None,
        )
        .await?;
    let coordinates = IngestionService::new(state)
        .ingest_bytes(
            IngestInput {
                filename: "homogeneous-coordinates.md".to_owned(),
                bytes: HOMOGENEOUS_COORDINATES.as_bytes().to_vec(),
                kind: crate::domain::SourceKind::File,
                title: Some("齐次坐标".to_owned()),
                uri: None,
                mime_type: Some("text/markdown".to_owned()),
            },
            None,
        )
        .await?;

    let course = CourseService::new(state)
        .create_course(
            RouteRequest {
                topic: DEMO_TOPIC.to_owned(),
                goal: Some("理解无穷远点、齐次坐标与射影平面".to_owned()),
                experience_level: None,
                desired_depth: None,
                source_ids: vec![plane.id, coordinates.id],
            },
            None,
        )
        .await?;

    let mut metadata: serde_json::Value =
        serde_json::from_str(&course.metadata_json).unwrap_or_else(|_| json!({}));
    metadata[DEMO_MARKER] = json!(true);
    sqlx::query("UPDATE courses SET metadata_json = ?, updated_at = ? WHERE id = ?")
        .bind(metadata.to_string())
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(course.id)
        .execute(&state.pool)
        .await?;

    let nodes = course_repo::list_nodes(&state.pool, course.id).await?;
    if let Some(first) = nodes.first() {
        if first.status != CourseNodeStatus::Completed {
            CourseService::new(state).complete_node(first.id).await?;
        }
    }

    Ok(course.id)
}
