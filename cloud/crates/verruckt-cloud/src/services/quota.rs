use crate::error::ApiError;
use crate::services::entitlements::{month_period_start, resolve_entitlements};
use crate::state::AppState;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use verruckt_cloud_domain::COUNTER_AI_CREDITS;
use verruckt_cloud_protocol::ai::{AiUsageEventDto, AiUsageResponse};

#[derive(Debug, Clone)]
pub struct AiUsageEventInput {
    pub operation: String,
    pub logical_model: String,
    pub upstream_model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub estimated_cost_units: i64,
    pub status: String,
    pub request_id: String,
    pub latency_ms: i64,
}

#[derive(sqlx::FromRow)]
struct AiUsageEventRow {
    id: Uuid,
    operation: String,
    logical_model: String,
    upstream_model: String,
    input_tokens: i64,
    output_tokens: i64,
    estimated_cost_units: i64,
    status: String,
    request_id: String,
    latency_ms: i64,
    created_at: DateTime<Utc>,
}

pub async fn reserve_ai_credits(
    state: &AppState,
    user_id: Uuid,
    logical_model: &str,
    estimated_units: i64,
    limit: i64,
    ttl_seconds: i64,
) -> Result<Uuid, ApiError> {
    if limit <= 0 {
        return Err(ApiError::payment_required(
            "ai_quota_exceeded",
            "the official AI quota has been exhausted",
        ));
    }
    let estimated_units = estimated_units.max(1);
    let period_start = month_period_start(Utc::now());
    let mut tx = state.pool.begin().await?;

    sqlx::query(
        r#"
        INSERT INTO usage_counters (id, user_id, counter_key, period_start, used)
        VALUES ($1, $2, $3, $4, 0)
        ON CONFLICT (user_id, counter_key, period_start) DO NOTHING
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(COUNTER_AI_CREDITS)
    .bind(period_start)
    .execute(&mut *tx)
    .await?;

    let used = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT used FROM usage_counters
        WHERE user_id = $1 AND counter_key = $2 AND period_start = $3
        FOR UPDATE
        "#,
    )
    .bind(user_id)
    .bind(COUNTER_AI_CREDITS)
    .bind(period_start)
    .fetch_one(&mut *tx)
    .await?;

    let reserved = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COALESCE(SUM(estimated_units), 0)::bigint
        FROM ai_reservations
        WHERE user_id = $1 AND status = 'active' AND expires_at > now()
        "#,
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;

    if used + reserved + estimated_units > limit {
        tx.rollback().await?;
        return Err(ApiError::payment_required(
            "ai_quota_exceeded",
            "the official AI quota has been exhausted",
        ));
    }

    let reservation_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO ai_reservations (id, user_id, logical_model, estimated_units, expires_at)
        VALUES ($1, $2, $3, $4, now() + ($5 || ' seconds')::interval)
        "#,
    )
    .bind(reservation_id)
    .bind(user_id)
    .bind(logical_model)
    .bind(estimated_units)
    .bind(ttl_seconds.to_string())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(reservation_id)
}

pub async fn commit_ai_usage(
    state: &AppState,
    user_id: Uuid,
    reservation_id: Uuid,
    actual_units: i64,
    event: AiUsageEventInput,
) -> Result<(), ApiError> {
    let actual_units = actual_units.max(0);
    let mut tx = state.pool.begin().await?;

    let reservation = sqlx::query_as::<_, (String, DateTime<Utc>)>(
        "SELECT status, created_at FROM ai_reservations WHERE id = $1 AND user_id = $2 FOR UPDATE",
    )
    .bind(reservation_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((status, created_at)) = reservation else {
        tx.rollback().await?;
        return Err(ApiError::internal_message(
            "AI reservation disappeared before commit",
        ));
    };
    if status != "active" {
        tx.rollback().await?;
        return Ok(());
    }

    sqlx::query(
        "UPDATE ai_reservations SET status = 'completed', completed_at = now() WHERE id = $1",
    )
    .bind(reservation_id)
    .execute(&mut *tx)
    .await?;

    let period_start = month_period_start(created_at);
    sqlx::query(
        r#"
        INSERT INTO usage_counters (id, user_id, counter_key, period_start, used)
        VALUES ($1, $2, $3, $4, 0)
        ON CONFLICT (user_id, counter_key, period_start) DO NOTHING
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(COUNTER_AI_CREDITS)
    .bind(period_start)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE usage_counters
        SET used = used + $4, updated_at = now()
        WHERE user_id = $1 AND counter_key = $2 AND period_start = $3
        "#,
    )
    .bind(user_id)
    .bind(COUNTER_AI_CREDITS)
    .bind(period_start)
    .bind(actual_units)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO ai_usage_events
            (id, user_id, operation, logical_model, upstream_model, input_tokens, output_tokens,
             estimated_cost_units, status, request_id, latency_ms)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(event.operation)
    .bind(event.logical_model)
    .bind(event.upstream_model)
    .bind(event.input_tokens)
    .bind(event.output_tokens)
    .bind(event.estimated_cost_units)
    .bind(event.status)
    .bind(event.request_id)
    .bind(event.latency_ms)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn release_ai_reservation(
    state: &AppState,
    user_id: Uuid,
    reservation_id: Uuid,
) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE ai_reservations SET status = 'released', completed_at = now() WHERE id = $1 AND user_id = $2 AND status = 'active'",
    )
    .bind(reservation_id)
    .bind(user_id)
    .execute(&state.pool)
    .await?;
    Ok(())
}

pub async fn get_ai_usage(state: &AppState, user_id: Uuid) -> Result<AiUsageResponse, ApiError> {
    let entitlements = resolve_entitlements(&state.pool, user_id).await?;
    let period_start = month_period_start(Utc::now());
    let credits_used = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COALESCE(used, 0) FROM usage_counters
        WHERE user_id = $1 AND counter_key = $2 AND period_start = $3
        "#,
    )
    .bind(user_id)
    .bind(COUNTER_AI_CREDITS)
    .bind(period_start)
    .fetch_optional(&state.pool)
    .await?
    .unwrap_or(0);

    let rows = sqlx::query_as::<_, AiUsageEventRow>(
        r#"
        SELECT id, operation, logical_model, upstream_model, input_tokens, output_tokens,
               estimated_cost_units, status, request_id, latency_ms, created_at
        FROM ai_usage_events
        WHERE user_id = $1
        ORDER BY created_at DESC
        LIMIT 50
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;

    let events = rows
        .into_iter()
        .map(|row| AiUsageEventDto {
            id: row.id,
            operation: row.operation,
            logical_model: row.logical_model,
            upstream_model: row.upstream_model,
            input_tokens: row.input_tokens,
            output_tokens: row.output_tokens,
            estimated_cost_units: row.estimated_cost_units,
            status: row.status,
            request_id: row.request_id,
            latency_ms: row.latency_ms,
            created_at: row.created_at,
        })
        .collect();

    let limit = entitlements.monthly_ai_credits();
    Ok(AiUsageResponse {
        period_start,
        credits_used,
        credits_limit: limit,
        credits_remaining: (limit - credits_used).max(0),
        events,
    })
}
