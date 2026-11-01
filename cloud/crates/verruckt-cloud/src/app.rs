use crate::error::ApiError;
use crate::request_id::request_id_middleware;
use crate::routes;
use crate::state::AppState;
use axum::http::{HeaderValue, Method, StatusCode};
use axum::routing::get;
use axum::Router;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

pub fn build_router(state: AppState) -> Router {
    let timeout = state.config.request_timeout();
    let max_body_bytes = state.config.max_body_bytes;
    let cors_origins = state.config.cors_origins.clone();

    let timed_routes = Router::new()
        .merge(routes::auth::router())
        .merge(routes::admin::router())
        .merge(routes::me::router())
        .merge(routes::entitlements::router())
        .merge(routes::devices::router())
        .merge(routes::sync::router())
        .merge(routes::files::router())
        .merge(routes::health::router())
        .route("/", get(|| async { "verruckt-cloud" }))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            timeout,
        ));

    let ai_routes = routes::ai::router();

    let mut router = timed_routes
        .merge(ai_routes)
        .fallback(fallback)
        .layer(axum::middleware::from_fn(request_id_middleware))
        .layer(TraceLayer::new_for_http())
        .layer(RequestBodyLimitLayer::new(max_body_bytes))
        .with_state(state);

    if !cors_origins.is_empty() {
        router = router.layer(cors_layer(&cors_origins));
    }

    router
}

async fn fallback() -> ApiError {
    ApiError::not_found("not_found", "the requested resource was not found")
}

fn cors_layer(origins: &[String]) -> CorsLayer {
    if origins.iter().any(|origin| origin == "*") {
        tracing::warn!("VERRUCKT_CORS_ORIGINS contains '*'; this is discouraged for production");
        return CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
            .allow_headers(Any);
    }

    let parsed = origins
        .iter()
        .filter_map(|origin| match HeaderValue::from_str(origin) {
            Ok(value) => Some(value),
            Err(_) => {
                tracing::warn!(origin = %origin, "ignoring invalid CORS origin");
                None
            }
        })
        .collect::<Vec<_>>();

    CorsLayer::new()
        .allow_origin(AllowOrigin::list(parsed))
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
        .allow_headers(Any)
}
