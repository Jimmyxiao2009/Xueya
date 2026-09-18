use crate::error::ApiError;
use crate::state::AppState;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use verruckt_cloud_protocol::health::{HealthResponse, ProviderStatus, ReadyResponse};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
}

async fn healthz() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        version: Some(env!("CARGO_PKG_VERSION").into()),
    })
}

async fn readyz(State(state): State<AppState>) -> Result<Json<ReadyResponse>, ApiError> {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "readiness database check failed");
            ApiError::service_unavailable("database_unavailable", "database is unavailable")
        })?;

    let migrations_ok =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM _sqlx_migrations)")
            .fetch_one(&state.pool)
            .await
            .unwrap_or(false);
    if !migrations_ok {
        return Err(ApiError::service_unavailable(
            "migrations_missing",
            "database migrations have not been applied",
        ));
    }

    let ai_status = if state.ai.is_some() {
        state.provider_status.ai_status().to_string()
    } else {
        "not_configured".into()
    };
    let status = if ai_status == "degraded" {
        "degraded"
    } else {
        "ready"
    };

    Ok(Json(ReadyResponse {
        status: status.into(),
        database: "ok".into(),
        migrations: "ok".into(),
        config: "ok".into(),
        providers: ProviderStatus { ai: ai_status },
    }))
}
