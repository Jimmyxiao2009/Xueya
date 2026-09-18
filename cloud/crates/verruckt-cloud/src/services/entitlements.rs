use crate::error::ApiError;
use crate::state::AppState;
use chrono::{DateTime, Datelike, TimeZone, Utc};
use sqlx::PgPool;
use uuid::Uuid;
use verruckt_cloud_domain::entitlements::{EntitlementSet, PlanEntitlement};
use verruckt_cloud_domain::{COUNTER_AI_CREDITS, PLAN_FREE};
use verruckt_cloud_protocol::entitlements::{EntitlementsResponse, EntitlementsUsage};

pub fn month_period_start(now: DateTime<Utc>) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .single()
        .unwrap_or(now)
}

pub async fn resolve_entitlements(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<EntitlementSet, ApiError> {
    let plan_id = sqlx::query_scalar::<_, String>(
        r#"
        SELECT plan_id FROM subscriptions
        WHERE user_id = $1 AND status = 'active'
        ORDER BY updated_at DESC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .unwrap_or_else(|| PLAN_FREE.into());

    let rows = sqlx::query_as::<_, (String, bool, Option<i64>)>(
        "SELECT key, enabled, limit_value FROM entitlements WHERE plan_id = $1",
    )
    .bind(&plan_id)
    .fetch_all(pool)
    .await?;

    Ok(EntitlementSet::from_rows(
        plan_id,
        rows.into_iter()
            .map(|(key, enabled, limit_value)| PlanEntitlement {
                key,
                enabled,
                limit_value,
            }),
    ))
}

pub async fn get_entitlements_response(
    state: &AppState,
    user_id: Uuid,
) -> Result<EntitlementsResponse, ApiError> {
    let entitlements = resolve_entitlements(&state.pool, user_id).await?;
    let period_start = month_period_start(Utc::now());

    let devices_used =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM devices WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&state.pool)
            .await?;

    let monthly_ai_credits_used = sqlx::query_scalar::<_, i64>(
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

    let cloud_storage_bytes_used = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(bytes_used, 0) FROM user_storage_usage WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    .unwrap_or(0);

    Ok(EntitlementsResponse {
        plan: entitlements.plan.clone(),
        features: entitlements.features.clone(),
        limits: entitlements.limits.clone(),
        usage: EntitlementsUsage {
            devices_used,
            monthly_ai_credits_used,
            cloud_storage_bytes_used,
        },
    })
}

pub async fn set_plan(state: &AppState, user_id: Uuid, plan_id: &str) -> Result<(), ApiError> {
    let plan_exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM plans WHERE id = $1)")
            .bind(plan_id)
            .fetch_one(&state.pool)
            .await?;
    if !plan_exists {
        return Err(ApiError::bad_request(
            "unknown_plan",
            format!("plan '{plan_id}' does not exist"),
        ));
    }

    sqlx::query(
        r#"
        INSERT INTO subscriptions (id, user_id, plan_id, status, current_period_start, current_period_end, updated_at)
        VALUES ($1, $2, $3, 'active', now(), now() + interval '30 days', now())
        ON CONFLICT (user_id) DO UPDATE
        SET plan_id = EXCLUDED.plan_id,
            status = 'active',
            current_period_start = EXCLUDED.current_period_start,
            current_period_end = EXCLUDED.current_period_end,
            updated_at = now()
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(plan_id)
    .execute(&state.pool)
    .await?;
    Ok(())
}
