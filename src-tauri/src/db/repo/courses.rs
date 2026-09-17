use crate::domain::{Course, CourseNode, CourseNodeDependency, CourseNodeStatus, RouteProgress};
use crate::error::AppError;
use sqlx::SqlitePool;
use uuid::Uuid;

const COURSE_COLUMNS: &str = "id, topic, title, description, status, metadata_json, goal, \
    experience_level, desired_depth, kind, current_node_id, seeded, created_at, updated_at";

const NODE_COLUMNS: &str = "id, course_id, title, description, objective, order_hint, \
    estimated_minutes, status, metadata_json, lesson_cache_json, lesson_cache_hash, kind, \
    confidence, review_at, generated_by, created_at, updated_at";

pub async fn insert_course_graph(
    pool: &SqlitePool,
    course: &Course,
    nodes: &[CourseNode],
    dependencies: &[CourseNodeDependency],
) -> Result<(), AppError> {
    let mut transaction = pool.begin().await?;

    sqlx::query(
        r#"
        INSERT INTO courses
            (id, topic, title, description, status, metadata_json, goal, experience_level,
             desired_depth, kind, current_node_id, seeded, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(course.id)
    .bind(&course.topic)
    .bind(&course.title)
    .bind(&course.description)
    .bind(course.status)
    .bind(&course.metadata_json)
    .bind(&course.goal)
    .bind(&course.experience_level)
    .bind(&course.desired_depth)
    .bind(&course.kind)
    .bind(course.current_node_id)
    .bind(course.seeded)
    .bind(course.created_at.to_rfc3339())
    .bind(course.updated_at.to_rfc3339())
    .execute(&mut *transaction)
    .await?;

    for node in nodes {
        insert_node(&mut transaction, node).await?;
    }

    for dependency in dependencies {
        sqlx::query(
            "INSERT OR IGNORE INTO course_node_dependencies (course_id, node_id, depends_on_node_id) VALUES (?, ?, ?)",
        )
        .bind(dependency.course_id)
        .bind(dependency.node_id)
        .bind(dependency.depends_on_node_id)
        .execute(&mut *transaction)
        .await?;
    }

    transaction.commit().await?;
    Ok(())
}

pub async fn insert_node(
    transaction: &mut sqlx::SqliteConnection,
    node: &CourseNode,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO course_nodes
            (id, course_id, title, description, objective, order_hint, estimated_minutes, status,
             metadata_json, lesson_cache_json, lesson_cache_hash, kind, confidence, review_at,
             generated_by, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(node.id)
    .bind(node.course_id)
    .bind(&node.title)
    .bind(&node.description)
    .bind(&node.objective)
    .bind(node.order_hint)
    .bind(node.estimated_minutes)
    .bind(node.status)
    .bind(&node.metadata_json)
    .bind(&node.lesson_cache_json)
    .bind(&node.lesson_cache_hash)
    .bind(&node.kind)
    .bind(node.confidence)
    .bind(node.review_at.map(|value| value.to_rfc3339()))
    .bind(&node.generated_by)
    .bind(node.created_at.to_rfc3339())
    .bind(node.updated_at.to_rfc3339())
    .execute(&mut *transaction)
    .await?;
    Ok(())
}

pub async fn list_courses(pool: &SqlitePool) -> Result<Vec<Course>, AppError> {
    let query = format!("SELECT {COURSE_COLUMNS} FROM courses ORDER BY created_at ASC");
    Ok(sqlx::query_as::<_, Course>(&query).fetch_all(pool).await?)
}

pub async fn get_course(pool: &SqlitePool, id: Uuid) -> Result<Course, AppError> {
    let query = format!("SELECT {COURSE_COLUMNS} FROM courses WHERE id = ?");
    sqlx::query_as::<_, Course>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("course {id}")))
}

pub async fn count_courses(pool: &SqlitePool) -> Result<i64, AppError> {
    Ok(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM courses")
        .fetch_one(pool)
        .await?)
}

pub async fn delete_course(pool: &SqlitePool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM courses WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_nodes(pool: &SqlitePool, course_id: Uuid) -> Result<Vec<CourseNode>, AppError> {
    let query = format!(
        "SELECT {NODE_COLUMNS} FROM course_nodes WHERE course_id = ? ORDER BY order_hint, created_at"
    );
    Ok(sqlx::query_as::<_, CourseNode>(&query)
        .bind(course_id)
        .fetch_all(pool)
        .await?)
}

pub async fn get_node(pool: &SqlitePool, id: Uuid) -> Result<CourseNode, AppError> {
    let query = format!("SELECT {NODE_COLUMNS} FROM course_nodes WHERE id = ?");
    sqlx::query_as::<_, CourseNode>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("course node {id}")))
}

pub async fn list_dependencies(
    pool: &SqlitePool,
    course_id: Uuid,
) -> Result<Vec<CourseNodeDependency>, AppError> {
    Ok(sqlx::query_as::<_, CourseNodeDependency>(
        "SELECT course_id, node_id, depends_on_node_id FROM course_node_dependencies WHERE course_id = ? ORDER BY node_id",
    )
    .bind(course_id)
    .fetch_all(pool)
    .await?)
}

pub async fn dependencies_for_node(
    pool: &SqlitePool,
    node_id: Uuid,
) -> Result<Vec<CourseNodeDependency>, AppError> {
    Ok(sqlx::query_as::<_, CourseNodeDependency>(
        "SELECT course_id, node_id, depends_on_node_id FROM course_node_dependencies WHERE node_id = ?",
    )
    .bind(node_id)
    .fetch_all(pool)
    .await?)
}

pub async fn update_node_status(
    pool: &SqlitePool,
    node_id: Uuid,
    status: CourseNodeStatus,
) -> Result<(), AppError> {
    let now = crate::util::time::utc_now().to_rfc3339();
    sqlx::query("UPDATE course_nodes SET status = ?, updated_at = ? WHERE id = ?")
        .bind(status)
        .bind(now)
        .bind(node_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_node_confidence(
    pool: &SqlitePool,
    node_id: Uuid,
    confidence: f64,
) -> Result<(), AppError> {
    let now = crate::util::time::utc_now().to_rfc3339();
    sqlx::query("UPDATE course_nodes SET confidence = ?, updated_at = ? WHERE id = ?")
        .bind(confidence.clamp(0.0, 1.0))
        .bind(now)
        .bind(node_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_node_lesson_cache(
    pool: &SqlitePool,
    node_id: Uuid,
    lesson_json: &str,
    source_hash: &str,
) -> Result<(), AppError> {
    let now = crate::util::time::utc_now().to_rfc3339();
    sqlx::query(
        "UPDATE course_nodes SET lesson_cache_json = ?, lesson_cache_hash = ?, updated_at = ? WHERE id = ?",
    )
    .bind(lesson_json)
    .bind(source_hash)
    .bind(now)
    .bind(node_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_current_node(
    pool: &SqlitePool,
    course_id: Uuid,
    node_id: Option<Uuid>,
) -> Result<(), AppError> {
    let now = crate::util::time::utc_now().to_rfc3339();
    sqlx::query("UPDATE courses SET current_node_id = ?, updated_at = ? WHERE id = ?")
        .bind(node_id)
        .bind(now)
        .bind(course_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn progress_for_course(
    pool: &SqlitePool,
    course_id: Uuid,
) -> Result<RouteProgress, AppError> {
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM course_nodes WHERE course_id = ?")
        .bind(course_id)
        .fetch_one(pool)
        .await?;
    let completed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM course_nodes WHERE course_id = ? AND status = 'completed'",
    )
    .bind(course_id)
    .fetch_one(pool)
    .await?;
    let current: Option<Uuid> =
        sqlx::query_scalar("SELECT current_node_id FROM courses WHERE id = ?")
            .bind(course_id)
            .fetch_optional(pool)
            .await?
            .flatten();
    Ok(RouteProgress {
        total,
        completed,
        current_node_id: current,
    })
}

/// The first node that is not completed, in roadmap order. This is what the
/// "从这里继续" card points at.
pub async fn first_incomplete_node(
    pool: &SqlitePool,
    course_id: Uuid,
) -> Result<Option<CourseNode>, AppError> {
    let query = format!(
        "SELECT {NODE_COLUMNS} FROM course_nodes WHERE course_id = ? AND status != 'completed' \
         ORDER BY order_hint, created_at LIMIT 1"
    );
    Ok(sqlx::query_as::<_, CourseNode>(&query)
        .bind(course_id)
        .fetch_optional(pool)
        .await?)
}

pub async fn insert_node_row(pool: &SqlitePool, node: &CourseNode) -> Result<(), AppError> {
    let mut connection = pool.acquire().await?;
    insert_node(&mut connection, node).await
}

pub async fn next_order_hint(pool: &SqlitePool, course_id: Uuid) -> Result<i64, AppError> {
    let value: Option<i64> =
        sqlx::query_scalar("SELECT MAX(order_hint) FROM course_nodes WHERE course_id = ?")
            .bind(course_id)
            .fetch_one(pool)
            .await?;
    Ok(value.unwrap_or(-1) + 1)
}

pub async fn shift_order_hints(
    pool: &SqlitePool,
    course_id: Uuid,
    from: i64,
) -> Result<(), AppError> {
    sqlx::query("UPDATE course_nodes SET order_hint = order_hint + 1 WHERE course_id = ? AND order_hint >= ?")
        .bind(course_id)
        .bind(from)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_review_nodes(
    pool: &SqlitePool,
    course_id: Uuid,
) -> Result<Vec<CourseNode>, AppError> {
    let query = format!(
        "SELECT {NODE_COLUMNS} FROM course_nodes WHERE course_id = ? AND kind = 'review' ORDER BY order_hint"
    );
    Ok(sqlx::query_as::<_, CourseNode>(&query)
        .bind(course_id)
        .fetch_all(pool)
        .await?)
}

pub async fn node_exists_for_title(
    pool: &SqlitePool,
    course_id: Uuid,
    title: &str,
) -> Result<bool, AppError> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM course_nodes WHERE course_id = ? AND title = ?")
            .bind(course_id)
            .bind(title)
            .fetch_one(pool)
            .await?;
    Ok(count > 0)
}

/// Flag a route as shipped demo content rather than something the learner made.
pub async fn mark_seeded(pool: &SqlitePool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("UPDATE courses SET seeded = 1, updated_at = ? WHERE id = ?")
        .bind(crate::util::time::utc_now().to_rfc3339())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
