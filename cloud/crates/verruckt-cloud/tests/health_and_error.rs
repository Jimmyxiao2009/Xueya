use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;
use verruckt_cloud::build_router;
use verruckt_cloud::config::Config;
use verruckt_cloud::state::AppState;

fn test_config() -> Config {
    Config {
        environment: "test".into(),
        bind: "127.0.0.1".parse().unwrap(),
        port: 0,
        database_url: "postgres://invalid:invalid@127.0.0.1:1/invalid".into(),
        access_token_secret: b"test-secret".to_vec(),
        token_issuer: "verruckt-cloud-test".into(),
        access_token_ttl_secs: 1800,
        refresh_token_ttl_days: 30,
        request_timeout_secs: 5,
        max_body_bytes: 1024,
        cors_origins: vec![],
        object_storage: None,
        ai: None,
        ai_rate_limit_per_minute: 60,
        ai_request_timeout_secs: 5,
        max_file_bytes: 1024,
    }
}

fn test_router() -> axum::Router {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://invalid:invalid@127.0.0.1:1/invalid")
        .expect("lazy pool");
    let state = AppState::new(pool, test_config(), None, None);
    build_router(state)
}

#[tokio::test]
async fn healthz_does_not_require_database() {
    let response = test_router()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn unknown_route_uses_error_envelope() {
    let response = test_router()
        .oneshot(
            Request::builder()
                .uri("/does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(response.headers().contains_key("x-request-id"));

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["error"]["code"], "not_found");
    assert_eq!(value["error"]["retryable"], false);
}
