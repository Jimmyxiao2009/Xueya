use crate::domain::{LearningAttempt, LearningSession, LearningSessionStatus, MasteryState};
use crate::error::AppError;
use sqlx::SqlitePool;
use uuid::Uuid;

pub async fn insert_session(pool: &SqlitePool, session: &LearningSession) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO learning_sessions (id, course_id, node_id, status, started_at, completed_at, summary_json)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(session.id)
    .bind(session.course_id)
    .bind(session.node_id)
    .bind(session.status)
    .bind(session.started_at.to_rfc3339())
    .bind(session.completed_at.map(|value| value.to_rfc3339()))
    .bind(&session.summary_json)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_session(pool: &SqlitePool, id: Uuid) -> Result<LearningSession, AppError> {
    sqlx::query_as::<_, LearningSession>(
        "SELECT id, course_id, node_id, status, started_at, completed_at, summary_json FROM learning_sessions WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("learning session {id}")))
}

pub async fn finish_session(
    pool: &SqlitePool,
    id: Uuid,
    summary_json: &str,
) -> Result<(), AppError> {
    let now = crate::util::time::utc_now().to_rfc3339();
    sqlx::query(
        "UPDATE learning_sessions SET status = ?, completed_at = ?, summary_json = ? WHERE id = ?",
    )
    .bind(LearningSessionStatus::Completed)
    .bind(now)
    .bind(summary_json)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_attempt(pool: &SqlitePool, attempt: &LearningAttempt) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO learning_attempts (id, session_id, node_id, user_answer, evaluation_json, correctness, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(attempt.id)
    .bind(attempt.session_id)
    .bind(attempt.node_id)
    .bind(&attempt.user_answer)
    .bind(&attempt.evaluation_json)
    .bind(attempt.correctness)
    .bind(attempt.created_at.to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_attempts_for_node(
    pool: &SqlitePool,
    node_id: Uuid,
) -> Result<Vec<LearningAttempt>, AppError> {
    Ok(sqlx::query_as::<_, LearningAttempt>(
        "SELECT id, session_id, node_id, user_answer, evaluation_json, correctness, created_at FROM learning_attempts WHERE node_id = ? ORDER BY created_at DESC LIMIT 20",
    )
    .bind(node_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_mastery(pool: &SqlitePool, node_id: Uuid) -> Result<MasteryState, AppError> {
    if let Some(state) = sqlx::query_as::<_, MasteryState>(
        "SELECT node_id, mastery, attempts, updated_at FROM mastery_states WHERE node_id = ?",
    )
    .bind(node_id)
    .fetch_optional(pool)
    .await?
    {
        return Ok(state);
    }
    Ok(MasteryState {
        node_id,
        mastery: 0.0,
        attempts: 0,
        updated_at: crate::util::time::utc_now(),
    })
}

pub async fn upsert_mastery(
    pool: &SqlitePool,
    node_id: Uuid,
    mastery: f64,
    attempts: i64,
) -> Result<(), AppError> {
    let now = crate::util::time::utc_now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO mastery_states (node_id, mastery, attempts, updated_at)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(node_id) DO UPDATE SET
            mastery = excluded.mastery,
            attempts = excluded.attempts,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(node_id)
    .bind(mastery.clamp(0.0, 1.0))
    .bind(attempts)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}
