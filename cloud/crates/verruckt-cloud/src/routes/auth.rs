use crate::auth::{self, AuthenticatedUser};
use crate::error::ApiError;
use crate::state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use verruckt_cloud_protocol::auth::{
    AuthResponse, LoginRequest, LogoutRequest, RefreshRequest, RegisterRequest,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/auth/register", post(register))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/refresh", post(refresh))
        .route("/api/v1/auth/logout", post(logout))
}

async fn register(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<AuthResponse>), ApiError> {
    let response = auth::register(&state, request).await?;
    Ok((StatusCode::CREATED, Json(response)))
}

async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    Ok(Json(auth::login(&state, request).await?))
}

async fn refresh(
    State(state): State<AppState>,
    Json(request): Json<RefreshRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    Ok(Json(auth::refresh(&state, request).await?))
}

async fn logout(
    State(state): State<AppState>,
    authenticated: AuthenticatedUser,
    body: Option<Json<LogoutRequest>>,
) -> Result<StatusCode, ApiError> {
    let refresh_token = body.and_then(|Json(body)| body.refresh_token);
    auth::logout(&state, authenticated, refresh_token).await?;
    Ok(StatusCode::NO_CONTENT)
}
