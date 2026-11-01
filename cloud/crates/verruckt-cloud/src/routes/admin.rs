use crate::{error::ApiError, state::AppState};
use axum::{
    extract::State,
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
fn authorize(headers: &HeaderMap) -> Result<(), ApiError> {
    let expected = std::env::var("ORBIT_ADMIN_TOKEN").unwrap_or_default();
    let provided = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    let difference = expected
        .bytes()
        .zip(provided.bytes())
        .fold(0u8, |v, (a, b)| v | (a ^ b));
    if expected.len() < 32 || expected.len() != provided.len() || difference != 0 {
        return Err(ApiError::unauthorized("admin_required", "需要管理员凭据"));
    }
    Ok(())
}
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/admin/overview", get(overview))
        .route("/api/admin/users/plan", post(set_plan))
}
async fn overview(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers)?;
    let users:Value=sqlx::query_scalar("SELECT COALESCE(jsonb_agg(r),'[]'::jsonb) FROM (SELECT u.id,u.email,u.created_at,(SELECT plan_id FROM subscriptions s WHERE s.user_id=u.id AND s.status='active' ORDER BY s.updated_at DESC LIMIT 1) AS plan,(SELECT count(*) FROM devices d WHERE d.user_id=u.id) AS devices FROM users u ORDER BY u.created_at DESC LIMIT 100) r").fetch_one(&state.pool).await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(
        json!({"users":users,"userCount":count,"database":"ok","officialAiConfigured":state.ai.is_some(),"storageConfigured":state.storage.is_some(),"aiStatus":state.provider_status.ai_status()}),
    ))
}
#[derive(Deserialize)]
struct PlanInput {
    email: String,
    plan: String,
}
async fn set_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<PlanInput>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers)?;
    if !matches!(input.plan.as_str(), "free" | "pro") {
        return Err(ApiError::bad_request("invalid_plan", "仅支持 free / pro"));
    }
    let id = crate::services::admin::set_plan(&state, &input.email, &input.plan).await?;
    Ok(Json(json!({"userId":id,"plan":input.plan})))
}
