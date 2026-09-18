use crate::auth::normalize_email;
use crate::error::ApiError;
use crate::services::entitlements::set_plan as set_user_plan;
use crate::state::AppState;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use verruckt_cloud_domain::password::hash_password;

#[derive(Debug, Clone)]
pub struct AdminUserReport {
    pub id: Uuid,
    pub email: String,
    pub plan: String,
    pub devices: i64,
    pub created_at: DateTime<Utc>,
}

pub async fn create_user(
    state: &AppState,
    email: &str,
    password: &str,
    plan: Option<&str>,
) -> Result<Uuid, ApiError> {
    let email_normalized = normalize_email(email)?;
    let password_hash = hash_password(password).map_err(|error| {
        ApiError::bad_request("invalid_password", format!("invalid password: {error}"))
    })?;

    let mut tx = state.pool.begin().await?;
    let user_id = Uuid::new_v4();
    let insert = sqlx::query(
        "INSERT INTO users (id, email, email_normalized, password_hash) VALUES ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(email.trim())
    .bind(&email_normalized)
    .bind(password_hash)
    .execute(&mut *tx)
    .await;

    match insert {
        Ok(_) => {}
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            return Err(ApiError::conflict(
                "email_already_registered",
                "an account with that email already exists",
            ));
        }
        Err(error) => return Err(error.into()),
    }

    sqlx::query(
        r#"
        INSERT INTO subscriptions (id, user_id, plan_id, status, current_period_start, current_period_end)
        VALUES ($1, $2, 'free', 'active', now(), now() + interval '30 days')
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    if let Some(plan) = plan {
        set_user_plan(state, user_id, plan).await?;
    }
    Ok(user_id)
}

pub async fn set_plan(state: &AppState, email: &str, plan: &str) -> Result<Uuid, ApiError> {
    let user_id = user_id_by_email(state, email).await?;
    set_user_plan(state, user_id, plan).await?;
    Ok(user_id)
}

pub async fn show_user(state: &AppState, email: &str) -> Result<AdminUserReport, ApiError> {
    let email_normalized = normalize_email(email)?;
    let row = sqlx::query_as::<_, (Uuid, String, DateTime<Utc>)>(
        "SELECT id, email, created_at FROM users WHERE email_normalized = $1",
    )
    .bind(email_normalized)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("user_not_found", "user not found"))?;

    let plan = sqlx::query_scalar::<_, String>(
        "SELECT plan_id FROM subscriptions WHERE user_id = $1 AND status = 'active' ORDER BY updated_at DESC LIMIT 1",
    )
    .bind(row.0)
    .fetch_optional(&state.pool)
    .await?
    .unwrap_or_else(|| verruckt_cloud_domain::PLAN_FREE.into());

    let devices = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM devices WHERE user_id = $1")
        .bind(row.0)
        .fetch_one(&state.pool)
        .await?;

    Ok(AdminUserReport {
        id: row.0,
        email: row.1,
        plan,
        devices,
        created_at: row.2,
    })
}

pub async fn reset_usage(state: &AppState, email: &str) -> Result<Uuid, ApiError> {
    let user_id = user_id_by_email(state, email).await?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("DELETE FROM usage_counters WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE ai_reservations SET status = 'released', completed_at = now() WHERE user_id = $1 AND status = 'active'",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(user_id)
}

async fn user_id_by_email(state: &AppState, email: &str) -> Result<Uuid, ApiError> {
    let email_normalized = normalize_email(email)?;
    sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE email_normalized = $1")
        .bind(email_normalized)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("user_not_found", "user not found"))
}
