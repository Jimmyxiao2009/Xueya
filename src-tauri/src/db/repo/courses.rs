use crate::domain::{Course, CourseNode, CourseNodeDependency, CourseNodeStatus};
use crate::error::AppError;
use sqlx::SqlitePool;
use uuid::Uuid;

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
            (id, topic, title, description, status, metadata_json, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(course.id)
    .bind(&course.topic)
    .bind(&course.title)
    .bind(&course.description)
    .bind(course.status)
    .bind(&course.metadata_json)
    .bind(course.created_at.to_rfc3339())
    .bind(course.updated_at.to_rfc3339())
    .execute(&mut *transaction)
    .await?;

    for node in nodes {
        sqlx::query(
            r#"
            INSERT INTO course_nodes
                (id, course_id, title, description, objective, order_hint, estimated_minutes, status, metadata_json, lesson_cache_json, lesson_cache_hash, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
        .bind(node.created_at.to_rfc3339())
        .bind(node.updated_at.to_rfc3339())
        .execute(&mut *transaction)
        .await?;
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

pub async fn list_courses(pool: &SqlitePool) -> Result<Vec<Course>, AppError> {
    Ok(sqlx::query_as::<_, Course>(
        "SELECT id, topic, title, description, status, metadata_json, created_at, updated_at FROM courses ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?)
}

pub async fn get_course(pool: &SqlitePool, id: Uuid) -> Result<Course, AppError> {
    sqlx::query_as::<_, Course>(
        "SELECT id, topic, title, description, status, metadata_json, created_at, updated_at FROM courses WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("course {id}")))
}

pub async fn delete_course(pool: &SqlitePool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM courses WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_nodes(pool: &SqlitePool, course_id: Uuid) -> Result<Vec<CourseNode>, AppError> {
    Ok(sqlx::query_as::<_, CourseNode>(
        "SELECT id, course_id, title, description, objective, order_hint, estimated_minutes, status, metadata_json, lesson_cache_json, lesson_cache_hash, created_at, updated_at FROM course_nodes WHERE course_id = ? ORDER BY order_hint, created_at",
    )
    .bind(course_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_node(pool: &SqlitePool, id: Uuid) -> Result<CourseNode, AppError> {
    sqlx::query_as::<_, CourseNode>(
        "SELECT id, course_id, title, description, objective, order_hint, estimated_minutes, status, metadata_json, lesson_cache_json, lesson_cache_hash, created_at, updated_at FROM course_nodes WHERE id = ?",
    )
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
