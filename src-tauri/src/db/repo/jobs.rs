use crate::domain::{Job, JobStatus};
use crate::error::AppError;
use sqlx::SqlitePool;
use uuid::Uuid;

pub async fn insert_job(pool: &SqlitePool, job: &Job) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO jobs (id, kind, status, progress, message, error_json, created_at, started_at, completed_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(job.id)
    .bind(&job.kind)
    .bind(job.status)
    .bind(job.progress)
    .bind(&job.message)
    .bind(&job.error_json)
    .bind(job.created_at.to_rfc3339())
    .bind(job.started_at.map(|value| value.to_rfc3339()))
    .bind(job.completed_at.map(|value| value.to_rfc3339()))
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_job(
    pool: &SqlitePool,
    id: Uuid,
    status: JobStatus,
    progress: f64,
    message: &str,
    error_json: Option<&str>,
    started_at: Option<chrono::DateTime<chrono::Utc>>,
    completed_at: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        UPDATE jobs SET
            status = ?,
            progress = ?,
            message = ?,
            error_json = COALESCE(?, error_json),
            started_at = COALESCE(?, started_at),
            completed_at = COALESCE(?, completed_at)
        WHERE id = ?
        "#,
    )
    .bind(status)
    .bind(progress)
    .bind(message)
    .bind(error_json)
    .bind(started_at.map(|value| value.to_rfc3339()))
    .bind(completed_at.map(|value| value.to_rfc3339()))
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_jobs(pool: &SqlitePool) -> Result<Vec<Job>, AppError> {
    Ok(sqlx::query_as::<_, Job>(
        "SELECT id, kind, status, progress, message, error_json, created_at, started_at, completed_at FROM jobs ORDER BY created_at DESC LIMIT 100",
    )
    .fetch_all(pool)
    .await?)
}

pub async fn get_job(pool: &SqlitePool, id: Uuid) -> Result<Job, AppError> {
    sqlx::query_as::<_, Job>(
        "SELECT id, kind, status, progress, message, error_json, created_at, started_at, completed_at FROM jobs WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("job {id}")))
}
