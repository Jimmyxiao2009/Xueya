//! Repository for the learning-loop tables introduced in migration 0002:
//! knowledge points, learner state, lesson plans, the quiz bank and the
//! adaptation log.

use crate::domain::{
    AdaptationEvent, KnowledgePoint, LearnerState, LearningMessageRecord, LessonStep, QuizAttempt,
    QuizQuestion,
};
use crate::error::AppError;
use sqlx::{Connection, SqlitePool};
use uuid::Uuid;

pub async fn insert_knowledge_points(
    pool: &SqlitePool,
    points: &[KnowledgePoint],
) -> Result<(), AppError> {
    let mut conn = pool.acquire().await?;
    insert_knowledge_points_tx(&mut conn, points).await
}

pub async fn insert_knowledge_points_tx(
    conn: &mut sqlx::SqliteConnection,
    points: &[KnowledgePoint],
) -> Result<(), AppError> {
    let mut transaction = conn.begin().await?;
    for point in points {
        sqlx::query(
            "INSERT OR REPLACE INTO knowledge_points (id, course_id, title, description, ordinal, created_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(point.id)
        .bind(point.course_id)
        .bind(&point.title)
        .bind(&point.description)
        .bind(point.ordinal)
        .bind(point.created_at.to_rfc3339())
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(())
}

pub async fn list_knowledge_points(
    pool: &SqlitePool,
    course_id: Uuid,
) -> Result<Vec<KnowledgePoint>, AppError> {
    Ok(sqlx::query_as::<_, KnowledgePoint>(
        "SELECT id, course_id, title, description, ordinal, created_at FROM knowledge_points \
         WHERE course_id = ? ORDER BY ordinal",
    )
    .bind(course_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_knowledge_point(pool: &SqlitePool, id: Uuid) -> Result<KnowledgePoint, AppError> {
    sqlx::query_as::<_, KnowledgePoint>(
        "SELECT id, course_id, title, description, ordinal, created_at FROM knowledge_points WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("knowledge point {id}")))
}

pub async fn link_node_points(
    pool: &SqlitePool,
    node_id: Uuid,
    point_ids: &[Uuid],
) -> Result<(), AppError> {
    let mut conn = pool.acquire().await?;
    link_node_points_tx(&mut conn, node_id, point_ids).await
}

pub async fn link_node_points_tx(
    conn: &mut sqlx::SqliteConnection,
    node_id: Uuid,
    point_ids: &[Uuid],
) -> Result<(), AppError> {
    let mut transaction = conn.begin().await?;
    for point_id in point_ids {
        sqlx::query(
            "INSERT OR IGNORE INTO node_knowledge_points (node_id, knowledge_point_id) VALUES (?, ?)",
        )
        .bind(node_id)
        .bind(point_id)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(())
}

pub async fn points_for_node(
    pool: &SqlitePool,
    node_id: Uuid,
) -> Result<Vec<KnowledgePoint>, AppError> {
    let mut conn = pool.acquire().await?;
    points_for_node_tx(&mut conn, node_id).await
}

pub async fn points_for_node_tx(
    conn: &mut sqlx::SqliteConnection,
    node_id: Uuid,
) -> Result<Vec<KnowledgePoint>, AppError> {
    Ok(sqlx::query_as::<_, KnowledgePoint>(
        "SELECT kp.id, kp.course_id, kp.title, kp.description, kp.ordinal, kp.created_at \
         FROM knowledge_points kp \
         JOIN node_knowledge_points nkp ON nkp.knowledge_point_id = kp.id \
         WHERE nkp.node_id = ? ORDER BY kp.ordinal",
    )
    .bind(node_id)
    .fetch_all(&mut *conn)
    .await?)
}

pub async fn replace_lesson_steps(
    pool: &SqlitePool,
    node_id: Uuid,
    steps: &[LessonStep],
) -> Result<(), AppError> {
    let mut conn = pool.acquire().await?;
    replace_lesson_steps_tx(&mut conn, node_id, steps).await
}

pub async fn replace_lesson_steps_tx(
    conn: &mut sqlx::SqliteConnection,
    node_id: Uuid,
    steps: &[LessonStep],
) -> Result<(), AppError> {
    let mut transaction = conn.begin().await?;
    sqlx::query("DELETE FROM lesson_steps WHERE node_id = ?")
        .bind(node_id)
        .execute(&mut *transaction)
        .await?;
    for step in steps {
        sqlx::query(
            "INSERT INTO lesson_steps (id, node_id, ordinal, title, detail, kind, duration_label, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(step.id)
        .bind(step.node_id)
        .bind(step.ordinal)
        .bind(&step.title)
        .bind(&step.detail)
        .bind(&step.kind)
        .bind(&step.duration_label)
        .bind(step.created_at.to_rfc3339())
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(())
}

pub async fn list_lesson_steps(
    pool: &SqlitePool,
    node_id: Uuid,
) -> Result<Vec<LessonStep>, AppError> {
    Ok(sqlx::query_as::<_, LessonStep>(
        "SELECT id, node_id, ordinal, title, detail, kind, duration_label, created_at \
         FROM lesson_steps WHERE node_id = ? ORDER BY ordinal",
    )
    .bind(node_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_learner_state(
    pool: &SqlitePool,
    course_id: Uuid,
    point_id: Uuid,
) -> Result<Option<LearnerState>, AppError> {
    let mut conn = pool.acquire().await?;
    get_learner_state_tx(&mut conn, course_id, point_id).await
}

pub async fn get_learner_state_tx(
    conn: &mut sqlx::SqliteConnection,
    course_id: Uuid,
    point_id: Uuid,
) -> Result<Option<LearnerState>, AppError> {
    Ok(sqlx::query_as::<_, LearnerState>(
        "SELECT course_id, knowledge_point_id, confidence, mastery, mistakes, attempts, \
         last_seen, next_review, updated_at FROM learner_states \
         WHERE course_id = ? AND knowledge_point_id = ?",
    )
    .bind(course_id)
    .bind(point_id)
    .fetch_optional(&mut *conn)
    .await?)
}

pub async fn upsert_learner_state(pool: &SqlitePool, state: &LearnerState) -> Result<(), AppError> {
    let mut conn = pool.acquire().await?;
    upsert_learner_state_tx(&mut conn, state).await
}

pub async fn upsert_learner_state_tx(
    conn: &mut sqlx::SqliteConnection,
    state: &LearnerState,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO learner_states \
            (course_id, knowledge_point_id, confidence, mastery, mistakes, attempts, last_seen, next_review, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(course_id, knowledge_point_id) DO UPDATE SET \
            confidence = excluded.confidence, mastery = excluded.mastery, \
            mistakes = excluded.mistakes, attempts = excluded.attempts, \
            last_seen = excluded.last_seen, next_review = excluded.next_review, \
            updated_at = excluded.updated_at",
    )
    .bind(state.course_id)
    .bind(state.knowledge_point_id)
    .bind(state.confidence)
    .bind(state.mastery)
    .bind(state.mistakes)
    .bind(state.attempts)
    .bind(state.last_seen.map(|value| value.to_rfc3339()))
    .bind(state.next_review.map(|value| value.to_rfc3339()))
    .bind(state.updated_at.to_rfc3339())
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub async fn list_learner_states(
    pool: &SqlitePool,
    course_id: Uuid,
) -> Result<Vec<LearnerState>, AppError> {
    Ok(sqlx::query_as::<_, LearnerState>(
        "SELECT course_id, knowledge_point_id, confidence, mastery, mistakes, attempts, \
         last_seen, next_review, updated_at FROM learner_states \
         WHERE course_id = ? ORDER BY updated_at DESC",
    )
    .bind(course_id)
    .fetch_all(pool)
    .await?)
}

pub async fn replace_quiz_questions(
    pool: &SqlitePool,
    node_id: Uuid,
    questions: &[QuizQuestion],
) -> Result<(), AppError> {
    let mut conn = pool.acquire().await?;
    replace_quiz_questions_tx(&mut conn, node_id, questions).await
}

pub async fn replace_quiz_questions_tx(
    conn: &mut sqlx::SqliteConnection,
    node_id: Uuid,
    questions: &[QuizQuestion],
) -> Result<(), AppError> {
    let mut transaction = conn.begin().await?;
    sqlx::query("DELETE FROM quiz_questions WHERE node_id = ?")
        .bind(node_id)
        .execute(&mut *transaction)
        .await?;
    for question in questions {
        sqlx::query(
            "INSERT INTO quiz_questions \
                (id, node_id, knowledge_point_id, ordinal, kind, prompt, options_json, answer_json, \
                 explanation, difficulty, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(question.id)
        .bind(question.node_id)
        .bind(question.knowledge_point_id)
        .bind(question.ordinal)
        .bind(&question.kind)
        .bind(&question.prompt)
        .bind(&question.options_json)
        .bind(&question.answer_json)
        .bind(&question.explanation)
        .bind(question.difficulty)
        .bind(question.created_at.to_rfc3339())
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(())
}

pub async fn list_quiz_questions(
    pool: &SqlitePool,
    node_id: Uuid,
) -> Result<Vec<QuizQuestion>, AppError> {
    Ok(sqlx::query_as::<_, QuizQuestion>(
        "SELECT id, node_id, knowledge_point_id, ordinal, kind, prompt, options_json, answer_json, \
         explanation, difficulty, created_at FROM quiz_questions WHERE node_id = ? ORDER BY ordinal",
    )
    .bind(node_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_quiz_question(pool: &SqlitePool, id: Uuid) -> Result<QuizQuestion, AppError> {
    sqlx::query_as::<_, QuizQuestion>(
        "SELECT id, node_id, knowledge_point_id, ordinal, kind, prompt, options_json, answer_json, \
         explanation, difficulty, created_at FROM quiz_questions WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("quiz question {id}")))
}

pub async fn insert_quiz_attempt(pool: &SqlitePool, attempt: &QuizAttempt) -> Result<(), AppError> {
    let mut conn = pool.acquire().await?;
    insert_quiz_attempt_tx(&mut conn, attempt).await
}

pub async fn insert_quiz_attempt_tx(
    conn: &mut sqlx::SqliteConnection,
    attempt: &QuizAttempt,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO quiz_attempts \
            (id, question_id, node_id, session_id, answer_json, correct, score, feedback, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(attempt.id)
    .bind(attempt.question_id)
    .bind(attempt.node_id)
    .bind(attempt.session_id)
    .bind(&attempt.answer_json)
    .bind(attempt.correct)
    .bind(attempt.score)
    .bind(&attempt.feedback)
    .bind(attempt.created_at.to_rfc3339())
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub async fn count_attempts_for_question(
    pool: &SqlitePool,
    question_id: Uuid,
) -> Result<i64, AppError> {
    Ok(
        sqlx::query_scalar("SELECT COUNT(*) FROM quiz_attempts WHERE question_id = ?")
            .bind(question_id)
            .fetch_one(pool)
            .await?,
    )
}

pub async fn insert_adaptation(pool: &SqlitePool, event: &AdaptationEvent) -> Result<(), AppError> {
    let mut conn = pool.acquire().await?;
    insert_adaptation_tx(&mut conn, event).await
}

pub async fn insert_adaptation_tx(
    conn: &mut sqlx::SqliteConnection,
    event: &AdaptationEvent,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO adaptation_events (id, course_id, node_id, kind, reason, payload_json, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(event.id)
    .bind(event.course_id)
    .bind(event.node_id)
    .bind(&event.kind)
    .bind(&event.reason)
    .bind(&event.payload_json)
    .bind(event.created_at.to_rfc3339())
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub async fn list_adaptations(
    pool: &SqlitePool,
    course_id: Uuid,
    limit: i64,
) -> Result<Vec<AdaptationEvent>, AppError> {
    let mut conn = pool.acquire().await?;
    list_adaptations_tx(&mut conn, course_id, limit).await
}

pub async fn list_adaptations_tx(
    conn: &mut sqlx::SqliteConnection,
    course_id: Uuid,
    limit: i64,
) -> Result<Vec<AdaptationEvent>, AppError> {
    Ok(sqlx::query_as::<_, AdaptationEvent>(
        "SELECT id, course_id, node_id, kind, reason, payload_json, created_at \
         FROM adaptation_events WHERE course_id = ? ORDER BY created_at DESC LIMIT ?",
    )
    .bind(course_id)
    .bind(limit)
    .fetch_all(&mut *conn)
    .await?)
}

pub async fn insert_message(
    pool: &SqlitePool,
    session_id: Uuid,
    role: &str,
    content: &str,
    metadata_json: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO learning_messages (id, session_id, role, content, metadata_json, created_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4())
    .bind(session_id)
    .bind(role)
    .bind(content)
    .bind(metadata_json)
    .bind(crate::util::time::utc_now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_messages(
    pool: &SqlitePool,
    session_id: Uuid,
) -> Result<Vec<(String, String)>, AppError> {
    Ok(sqlx::query_as::<_, (String, String)>(
        "SELECT role, content FROM learning_messages WHERE session_id = ? ORDER BY created_at",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?)
}

/// Return persisted messages with metadata so the service layer can assemble
/// first-class [`crate::domain::LearningTurn`] values.
pub async fn list_message_records(
    pool: &SqlitePool,
    session_id: Uuid,
) -> Result<Vec<LearningMessageRecord>, AppError> {
    Ok(sqlx::query_as::<_, LearningMessageRecord>(
        "SELECT id, session_id, role, content, metadata_json, created_at \
         FROM learning_messages WHERE session_id = ? ORDER BY created_at, rowid",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?)
}
