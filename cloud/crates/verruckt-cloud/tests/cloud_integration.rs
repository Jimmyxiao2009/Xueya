use axum::body::Body;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use chrono::Utc;
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use uuid::Uuid;
use verruckt_cloud::build_router;
use verruckt_cloud::config::{AiConfig, Config};
use verruckt_cloud::services::admin;
use verruckt_cloud::services::ai::AiGateway;
use verruckt_cloud::services::entitlements::month_period_start;
use verruckt_cloud::state::AppState;
use verruckt_cloud_storage::{ObjectStorageConfig, S3ObjectStorage};

const TEST_DB_ENV: &str = "VERRUCKT_TEST_DATABASE_URL";
const PASSWORD: &str = "correct-horse-battery-staple";

#[derive(Clone, Copy)]
enum MockMode {
    Success,
    Unauthorized,
    RateLimited,
}

#[derive(Clone)]
struct MockState {
    mode: MockMode,
}

#[tokio::test]
async fn auth_refresh_logout_and_device_limit() {
    let Some(pool) = test_pool().await else {
        eprintln!("skipping: set {TEST_DB_ENV} to run integration tests");
        return;
    };
    let (base, server) = spawn_app(test_state(pool.clone(), None, None)).await;
    let client = reqwest::Client::new();
    let email = unique_email("auth");

    let register = client
        .post(format!("{base}/api/v1/auth/register"))
        .json(&json!({
            "email": email,
            "password": PASSWORD,
            "device": {"name": "Device 1", "platform": "windows", "app_version": "0.1.0"}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(register.status(), reqwest::StatusCode::CREATED);
    let register_body: Value = register.json().await.unwrap();
    let device_id = register_body["device"]["id"].as_str().unwrap().to_string();

    let duplicate = client
        .post(format!("{base}/api/v1/auth/register"))
        .json(&json!({"email": email, "password": PASSWORD}))
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate.status(), reqwest::StatusCode::CONFLICT);

    let wrong = client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&json!({"email": email, "password": "wrong-password-123"}))
        .send()
        .await
        .unwrap();
    assert_eq!(wrong.status(), reqwest::StatusCode::UNAUTHORIZED);

    let login = client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&json!({
            "email": email,
            "password": PASSWORD,
            "device_id": device_id,
            "device": {"name": "Device 1", "platform": "windows", "app_version": "0.2.0"}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), reqwest::StatusCode::OK);
    let login_body: Value = login.json().await.unwrap();
    let access = login_body["access_token"].as_str().unwrap().to_string();
    let refresh = login_body["refresh_token"].as_str().unwrap().to_string();

    let me = client
        .get(format!("{base}/api/v1/me"))
        .bearer_auth(&access)
        .send()
        .await
        .unwrap();
    assert_eq!(me.status(), reqwest::StatusCode::OK);

    let rotated = client
        .post(format!("{base}/api/v1/auth/refresh"))
        .json(&json!({"refresh_token": refresh}))
        .send()
        .await
        .unwrap();
    assert_eq!(rotated.status(), reqwest::StatusCode::OK);
    let rotated_body: Value = rotated.json().await.unwrap();
    let new_access = rotated_body["access_token"].as_str().unwrap().to_string();

    let reuse = client
        .post(format!("{base}/api/v1/auth/refresh"))
        .json(&json!({"refresh_token": refresh}))
        .send()
        .await
        .unwrap();
    assert_eq!(reuse.status(), reqwest::StatusCode::UNAUTHORIZED);
    let reuse_body: Value = reuse.json().await.unwrap();
    assert_eq!(reuse_body["error"]["code"], "refresh_token_reuse_detected");

    let after_reuse = client
        .get(format!("{base}/api/v1/me"))
        .bearer_auth(&new_access)
        .send()
        .await
        .unwrap();
    assert_eq!(after_reuse.status(), reqwest::StatusCode::UNAUTHORIZED);

    let limit_email = unique_email("device-limit");
    client
        .post(format!("{base}/api/v1/auth/register"))
        .json(&json!({
            "email": limit_email,
            "password": PASSWORD,
            "device": {"name": "Only Device", "platform": "macos", "app_version": "0.1.0"}
        }))
        .send()
        .await
        .unwrap();
    let second_device = client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&json!({
            "email": limit_email,
            "password": PASSWORD,
            "device": {"name": "Second Device", "platform": "linux", "app_version": "0.1.0"}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(second_device.status(), reqwest::StatusCode::FORBIDDEN);
    let second_body: Value = second_device.json().await.unwrap();
    assert_eq!(second_body["error"]["code"], "device_limit_reached");

    server.abort();
}

#[tokio::test]
async fn sync_push_pull_conflict_idempotency_and_isolation() {
    let Some(pool) = test_pool().await else {
        eprintln!("skipping: set {TEST_DB_ENV} to run integration tests");
        return;
    };
    let state = test_state(pool.clone(), None, None);
    let (base, server) = spawn_app(state.clone()).await;
    let client = reqwest::Client::new();

    let user_a = unique_email("sync-a");
    let user_b = unique_email("sync-b");
    admin::create_user(&state, &user_a, PASSWORD, Some("pro"))
        .await
        .unwrap();
    admin::create_user(&state, &user_b, PASSWORD, Some("pro"))
        .await
        .unwrap();

    let a_device_1 = login(&client, &base, &user_a, "A Device 1").await;
    let a_device_2 = login(&client, &base, &user_a, "A Device 2").await;
    let b_device = login(&client, &base, &user_b, "B Device 1").await;
    let a_access_1 = a_device_1["access_token"].as_str().unwrap();
    let a_access_2 = a_device_2["access_token"].as_str().unwrap();
    let a_device_id_1 = a_device_1["device"]["id"].as_str().unwrap();
    let a_device_id_2 = a_device_2["device"]["id"].as_str().unwrap();
    let b_access = b_device["access_token"].as_str().unwrap();
    let b_device_id = b_device["device"]["id"].as_str().unwrap();

    let entity_id = Uuid::new_v4();
    let change_id = Uuid::new_v4();
    let push_body = json!({
        "device_id": a_device_id_1,
        "changes": [{
            "change_id": change_id,
            "entity_type": "KnowledgeNode",
            "entity_id": entity_id,
            "base_revision": 0,
            "revision": 1,
            "operation": "upsert",
            "updated_at": Utc::now().to_rfc3339(),
            "payload": {"title": "A node"}
        }]
    });

    let push = client
        .post(format!("{base}/api/v1/sync/push"))
        .bearer_auth(a_access_1)
        .json(&push_body)
        .send()
        .await
        .unwrap();
    assert_eq!(push.status(), reqwest::StatusCode::OK);
    let push_body_value: Value = push.json().await.unwrap();
    assert_eq!(push_body_value["accepted"][0]["revision"], 1);
    assert_eq!(push_body_value["conflicts"].as_array().unwrap().len(), 0);

    let retry = client
        .post(format!("{base}/api/v1/sync/push"))
        .bearer_auth(a_access_1)
        .json(&push_body)
        .send()
        .await
        .unwrap();
    let retry_body: Value = retry.json().await.unwrap();
    assert_eq!(retry_body["accepted"][0]["already_applied"], true);

    let pull = client
        .get(format!("{base}/api/v1/sync/pull?limit=100"))
        .bearer_auth(a_access_2)
        .send()
        .await
        .unwrap();
    assert_eq!(pull.status(), reqwest::StatusCode::OK);
    let pull_body: Value = pull.json().await.unwrap();
    assert_eq!(pull_body["changes"].as_array().unwrap().len(), 1);
    assert_eq!(pull_body["changes"][0]["entity_id"], entity_id.to_string());
    assert_eq!(pull_body["changes"][0]["revision"], 1);

    let conflict = client
        .post(format!("{base}/api/v1/sync/push"))
        .bearer_auth(a_access_1)
        .json(&json!({
            "device_id": a_device_id_1,
            "changes": [{
                "change_id": Uuid::new_v4(),
                "entity_type": "KnowledgeNode",
                "entity_id": entity_id,
                "base_revision": 0,
                "revision": 2,
                "operation": "upsert",
                "updated_at": Utc::now().to_rfc3339(),
                "payload": {"title": "stale"}
            }]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(conflict.status(), reqwest::StatusCode::OK);
    let conflict_body: Value = conflict.json().await.unwrap();
    assert_eq!(conflict_body["conflicts"][0]["server_version"], 1);
    assert_eq!(conflict_body["conflicts"][0]["client_version"], 2);
    assert_eq!(
        conflict_body["conflicts"][0]["server_payload"]["title"],
        "A node"
    );

    let delete = client
        .post(format!("{base}/api/v1/sync/push"))
        .bearer_auth(a_access_2)
        .json(&json!({
            "device_id": a_device_id_2,
            "changes": [{
                "change_id": Uuid::new_v4(),
                "entity_type": "KnowledgeNode",
                "entity_id": entity_id,
                "base_revision": 1,
                "revision": 2,
                "operation": "delete",
                "updated_at": Utc::now().to_rfc3339(),
                "payload": {}
            }]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(delete.status(), reqwest::StatusCode::OK);

    let bootstrap = client
        .get(format!("{base}/api/v1/sync/bootstrap?limit=100"))
        .bearer_auth(a_access_1)
        .send()
        .await
        .unwrap();
    let bootstrap_body: Value = bootstrap.json().await.unwrap();
    let entity_id_string = entity_id.to_string();
    let tombstone = bootstrap_body["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entity| entity["entity_id"].as_str() == Some(entity_id_string.as_str()))
        .unwrap();
    assert_eq!(tombstone["deleted"], true);

    let b_entity_id = Uuid::new_v4();
    let b_push = client
        .post(format!("{base}/api/v1/sync/push"))
        .bearer_auth(b_access)
        .json(&json!({
            "device_id": b_device_id,
            "changes": [{
                "change_id": Uuid::new_v4(),
                "entity_type": "KnowledgeNode",
                "entity_id": b_entity_id,
                "base_revision": 0,
                "revision": 1,
                "operation": "upsert",
                "updated_at": Utc::now().to_rfc3339(),
                "payload": {"title": "B private"}
            }]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(b_push.status(), reqwest::StatusCode::OK);

    let a_bootstrap = client
        .get(format!("{base}/api/v1/sync/bootstrap?limit=100"))
        .bearer_auth(a_access_1)
        .send()
        .await
        .unwrap();
    let a_bootstrap_body: Value = a_bootstrap.json().await.unwrap();
    let b_entity_id_string = b_entity_id.to_string();
    assert!(!a_bootstrap_body["entities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entity| entity["entity_id"].as_str() == Some(b_entity_id_string.as_str())));

    server.abort();
}

#[tokio::test]
async fn ai_gateway_chat_embeddings_stream_and_quota() {
    let Some(pool) = test_pool().await else {
        eprintln!("skipping: set {TEST_DB_ENV} to run integration tests");
        return;
    };
    let (mock_base, mock_server) = spawn_mock(MockMode::Success).await;
    let ai = Some(test_ai_gateway(&mock_base));
    let state = test_state(pool.clone(), ai, None);
    let (base, server) = spawn_app(state.clone()).await;
    let client = reqwest::Client::new();

    let email = unique_email("ai");
    admin::create_user(&state, &email, PASSWORD, Some("pro"))
        .await
        .unwrap();
    let session = login(&client, &base, &email, "AI Device").await;
    let access = session["access_token"].as_str().unwrap();

    let chat = client
        .post(format!("{base}/v1/chat/completions"))
        .bearer_auth(access)
        .json(&json!({
            "model": "verruckt-fast",
            "messages": [{"role": "user", "content": "hello"}],
            "max_tokens": 32
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(chat.status(), reqwest::StatusCode::OK);
    let chat_body: Value = chat.json().await.unwrap();
    assert_eq!(chat_body["model"], "verruckt-fast");
    assert_eq!(chat_body["usage"]["total_tokens"], 8);

    let embeddings = client
        .post(format!("{base}/v1/embeddings"))
        .bearer_auth(access)
        .json(&json!({
            "model": "verruckt-embedding",
            "input": ["hello", "world"]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(embeddings.status(), reqwest::StatusCode::OK);
    let embeddings_body: Value = embeddings.json().await.unwrap();
    assert_eq!(embeddings_body["model"], "verruckt-embedding");

    let stream = client
        .post(format!("{base}/api/v1/ai/chat/stream"))
        .bearer_auth(access)
        .json(&json!({
            "model": "verruckt-fast",
            "messages": [{"role": "user", "content": "stream"}],
            "max_tokens": 32
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(stream.status(), reqwest::StatusCode::OK);
    assert_eq!(
        stream
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream")
    );
    let stream_body = stream.text().await.unwrap();
    assert!(stream_body.contains("data: "));
    assert!(stream_body.contains("[DONE]"));

    let usage = client
        .get(format!("{base}/api/v1/ai/usage"))
        .bearer_auth(access)
        .send()
        .await
        .unwrap();
    assert_eq!(usage.status(), reqwest::StatusCode::OK);
    let usage_body: Value = usage.json().await.unwrap();
    assert!(usage_body["credits_used"].as_i64().unwrap() > 0);
    assert!(usage_body["events"].as_array().unwrap().len() >= 3);

    let free_email = unique_email("ai-free");
    admin::create_user(&state, &free_email, PASSWORD, Some("free"))
        .await
        .unwrap();
    let free_session = login(&client, &base, &free_email, "Free Device").await;
    let free_access = free_session["access_token"].as_str().unwrap();
    let denied = client
        .post(format!("{base}/v1/chat/completions"))
        .bearer_auth(free_access)
        .json(&json!({
            "model": "verruckt-fast",
            "messages": [{"role": "user", "content": "hello"}],
            "max_tokens": 8
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), reqwest::StatusCode::FORBIDDEN);
    let denied_body: Value = denied.json().await.unwrap();
    assert_eq!(denied_body["error"]["code"], "entitlement_required");

    let period_start = month_period_start(Utc::now());
    sqlx::query(
        r#"
        INSERT INTO usage_counters (id, user_id, counter_key, period_start, used)
        VALUES ($1, (SELECT id FROM users WHERE email_normalized = $2), 'ai_credits', $3, 100000)
        ON CONFLICT (user_id, counter_key, period_start)
        DO UPDATE SET used = EXCLUDED.used
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(&email)
    .bind(period_start)
    .execute(&pool)
    .await
    .unwrap();

    let exhausted = client
        .post(format!("{base}/v1/chat/completions"))
        .bearer_auth(access)
        .json(&json!({
            "model": "verruckt-fast",
            "messages": [{"role": "user", "content": "hello"}],
            "max_tokens": 8
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(exhausted.status(), reqwest::StatusCode::PAYMENT_REQUIRED);
    let exhausted_body: Value = exhausted.json().await.unwrap();
    assert_eq!(exhausted_body["error"]["code"], "ai_quota_exceeded");

    server.abort();
    mock_server.abort();
}

#[tokio::test]
async fn ai_upstream_errors_are_mapped_safely() {
    let Some(pool) = test_pool().await else {
        eprintln!("skipping: set {TEST_DB_ENV} to run integration tests");
        return;
    };
    for (mode, expected_status, expected_code) in [
        (
            MockMode::Unauthorized,
            reqwest::StatusCode::BAD_GATEWAY,
            "upstream_auth_failed",
        ),
        (
            MockMode::RateLimited,
            reqwest::StatusCode::TOO_MANY_REQUESTS,
            "upstream_rate_limited",
        ),
    ] {
        let (mock_base, mock_server) = spawn_mock(mode).await;
        let state = test_state(pool.clone(), Some(test_ai_gateway(&mock_base)), None);
        let (base, server) = spawn_app(state.clone()).await;
        let client = reqwest::Client::new();
        let email = unique_email("ai-errors");
        admin::create_user(&state, &email, PASSWORD, Some("pro"))
            .await
            .unwrap();
        let session = login(&client, &base, &email, "Error Device").await;
        let access = session["access_token"].as_str().unwrap();
        let response = client
            .post(format!("{base}/v1/chat/completions"))
            .bearer_auth(access)
            .json(&json!({
                "model": "verruckt-fast",
                "messages": [{"role": "user", "content": "hello"}],
                "max_tokens": 8
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), expected_status);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["error"]["code"], expected_code);
        server.abort();
        mock_server.abort();
    }
}

#[tokio::test]
async fn files_presign_ownership_dedup_and_quota() {
    let Some(pool) = test_pool().await else {
        eprintln!("skipping: set {TEST_DB_ENV} to run integration tests");
        return;
    };
    let Some(storage) = test_storage() else {
        eprintln!("skipping file test: set VERRUCKT_TEST_OBJECT_STORAGE_ENDPOINT to run");
        return;
    };
    storage.ensure_bucket().await.unwrap();
    let state = test_state(pool.clone(), None, Some(storage));
    let (base, server) = spawn_app(state.clone()).await;
    let client = reqwest::Client::new();

    let email_a = unique_email("files-a");
    let email_b = unique_email("files-b");
    admin::create_user(&state, &email_a, PASSWORD, Some("pro"))
        .await
        .unwrap();
    admin::create_user(&state, &email_b, PASSWORD, Some("pro"))
        .await
        .unwrap();
    let session_a = login(&client, &base, &email_a, "Files A").await;
    let session_b = login(&client, &base, &email_b, "Files B").await;
    let access_a = session_a["access_token"].as_str().unwrap();
    let access_b = session_b["access_token"].as_str().unwrap();

    use sha2::{Digest, Sha256};
    let data = vec![b'a'; 123];
    let sha = hex::encode(Sha256::digest(&data));
    let upload = client
        .post(format!("{base}/api/v1/files/presign-upload"))
        .bearer_auth(access_a)
        .json(&json!({"sha256": sha, "size": 123, "mime": "text/plain", "filename": "note.txt"}))
        .send()
        .await
        .unwrap();
    assert_eq!(upload.status(), reqwest::StatusCode::OK);
    let upload_body: Value = upload.json().await.unwrap();
    let file_id = upload_body["file_id"].as_str().unwrap();
    assert_eq!(upload_body["deduplicated"], false);
    assert!(upload_body["object_key"]
        .as_str()
        .unwrap()
        .contains("/objects/"));

    let mut put = client.put(upload_body["upload_url"].as_str().unwrap());
    for (key, value) in upload_body["headers"].as_object().unwrap() {
        put = put.header(key, value.as_str().unwrap());
    }
    assert!(put.body(data).send().await.unwrap().status().is_success());
    assert!(client
        .post(format!("{base}/api/v1/files/{file_id}/complete"))
        .bearer_auth(access_a)
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .status()
        .is_success());

    let dedup = client
        .post(format!("{base}/api/v1/files/presign-upload"))
        .bearer_auth(access_a)
        .json(&json!({"sha256": sha, "size": 123, "mime": "text/plain"}))
        .send()
        .await
        .unwrap();
    assert_eq!(dedup.status(), reqwest::StatusCode::OK);
    let dedup_body: Value = dedup.json().await.unwrap();
    assert_eq!(dedup_body["deduplicated"], true);
    assert_eq!(dedup_body["file_id"], file_id);

    let foreign_download = client
        .post(format!("{base}/api/v1/files/presign-download"))
        .bearer_auth(access_b)
        .json(&json!({"file_id": file_id}))
        .send()
        .await
        .unwrap();
    assert_eq!(foreign_download.status(), reqwest::StatusCode::NOT_FOUND);

    let own_download = client
        .post(format!("{base}/api/v1/files/presign-download"))
        .bearer_auth(access_a)
        .json(&json!({"file_id": file_id}))
        .send()
        .await
        .unwrap();
    assert_eq!(own_download.status(), reqwest::StatusCode::OK);
    let own_download_body: Value = own_download.json().await.unwrap();
    assert!(own_download_body["download_url"]
        .as_str()
        .unwrap()
        .contains("X-Amz-Signature="));

    let foreign_delete = client
        .delete(format!("{base}/api/v1/files/{file_id}"))
        .bearer_auth(access_b)
        .send()
        .await
        .unwrap();
    assert_eq!(foreign_delete.status(), reqwest::StatusCode::NOT_FOUND);

    let free_email = unique_email("files-free");
    admin::create_user(&state, &free_email, PASSWORD, Some("free"))
        .await
        .unwrap();
    let free_session = login(&client, &base, &free_email, "Files Free").await;
    let free_access = free_session["access_token"].as_str().unwrap();
    let denied = client
        .post(format!("{base}/api/v1/files/presign-upload"))
        .bearer_auth(free_access)
        .json(&json!({"sha256": "b".repeat(64), "size": 1, "mime": "text/plain"}))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), reqwest::StatusCode::FORBIDDEN);

    server.abort();
}

async fn test_pool() -> Option<PgPool> {
    let url = std::env::var(TEST_DB_ENV).ok()?;
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&url)
        .await
        .expect("connect test database");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("run test migrations");
    Some(pool)
}

fn test_state(
    pool: PgPool,
    ai: Option<Arc<AiGateway>>,
    storage: Option<Arc<S3ObjectStorage>>,
) -> AppState {
    AppState::new(pool, test_config(), storage, ai)
}

fn test_config() -> Config {
    let database_url = std::env::var(TEST_DB_ENV)
        .unwrap_or_else(|_| "postgres://invalid:invalid@127.0.0.1:1/invalid".into());
    Config {
        environment: "test".into(),
        bind: "127.0.0.1".parse().unwrap(),
        port: 0,
        database_url,
        access_token_secret: b"integration-test-secret".to_vec(),
        token_issuer: "verruckt-cloud-test".into(),
        access_token_ttl_secs: 1800,
        refresh_token_ttl_days: 30,
        request_timeout_secs: 30,
        max_body_bytes: 1024 * 1024,
        cors_origins: vec![],
        object_storage: None,
        ai: None,
        ai_rate_limit_per_minute: 600,
        ai_request_timeout_secs: 10,
        max_file_bytes: 1024 * 1024,
    }
}

fn test_ai_gateway(base_url: &str) -> Arc<AiGateway> {
    let mut models = BTreeMap::new();
    models.insert("verruckt-fast".into(), "upstream-chat".into());
    models.insert("verruckt-standard".into(), "upstream-chat".into());
    models.insert("verruckt-reasoning".into(), "upstream-reasoning".into());
    models.insert("verruckt-embedding".into(), "upstream-embedding".into());
    Arc::new(
        AiGateway::new(
            AiConfig {
                base_url: base_url.into(),
                api_key: "test-key".into(),
                models,
                max_messages: 64,
                max_tokens: 8192,
                max_embedding_inputs: 128,
                max_embedding_total_chars: 131072,
            },
            Duration::from_secs(10),
        )
        .unwrap(),
    )
}

fn test_storage() -> Option<Arc<S3ObjectStorage>> {
    let endpoint = std::env::var("VERRUCKT_TEST_OBJECT_STORAGE_ENDPOINT").ok()?;
    S3ObjectStorage::new(ObjectStorageConfig {
        endpoint,
        region: "us-east-1".into(),
        bucket: std::env::var("VERRUCKT_TEST_OBJECT_STORAGE_BUCKET")
            .unwrap_or_else(|_| "verruckt-cloud".into()),
        access_key: std::env::var("VERRUCKT_TEST_OBJECT_STORAGE_ACCESS_KEY")
            .unwrap_or_else(|_| "minioadmin".into()),
        secret_key: std::env::var("VERRUCKT_TEST_OBJECT_STORAGE_SECRET_KEY")
            .unwrap_or_else(|_| "minioadmin".into()),
        force_path_style: true,
        presign_ttl_secs: 900,
    })
    .ok()
    .map(Arc::new)
}

async fn spawn_app(state: AppState) -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = build_router(state);
    let handle = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (format!("http://{address}"), handle)
}

async fn login(client: &reqwest::Client, base: &str, email: &str, device_name: &str) -> Value {
    let response = client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&json!({
            "email": email,
            "password": PASSWORD,
            "device": {"name": device_name, "platform": "test", "app_version": "0.1.0"}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    response.json().await.unwrap()
}

async fn spawn_mock(mode: MockMode) -> (String, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route("/chat/completions", post(mock_chat))
        .route("/embeddings", post(mock_embeddings))
        .with_state(MockState { mode });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{address}"), handle)
}

async fn mock_chat(State(state): State<MockState>, Json(body): Json<Value>) -> Response {
    match state.mode {
        MockMode::Unauthorized => (
            reqwest::StatusCode::UNAUTHORIZED,
            Json(json!({"error": "invalid upstream key"})),
        )
            .into_response(),
        MockMode::RateLimited => (
            reqwest::StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "slow down"})),
        )
            .into_response(),
        MockMode::Success => {
            let stream = body.get("stream").and_then(Value::as_bool).unwrap_or(false);
            if stream {
                let sse = concat!(
                    "data: {\"id\":\"chatcmpl-1\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hel\"}}]}\n\n",
                    "data: {\"id\":\"chatcmpl-1\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"lo\"}}]}\n\n",
                    "data: {\"id\":\"chatcmpl-1\",\"choices\":[],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":3,\"total_tokens\":8}}\n\n",
                    "data: [DONE]\n\n"
                );
                Response::builder()
                    .status(reqwest::StatusCode::OK)
                    .header("content-type", "text/event-stream")
                    .body(Body::from(sse))
                    .unwrap()
            } else {
                Json(json!({
                    "id": "chatcmpl-1",
                    "object": "chat.completion",
                    "model": body.get("model").cloned().unwrap_or(Value::Null),
                    "choices": [{
                        "index": 0,
                        "message": {"role": "assistant", "content": "hello"},
                        "finish_reason": "stop"
                    }],
                    "usage": {"prompt_tokens": 5, "completion_tokens": 3, "total_tokens": 8}
                }))
                .into_response()
            }
        }
    }
}

async fn mock_embeddings(State(state): State<MockState>, Json(body): Json<Value>) -> Response {
    match state.mode {
        MockMode::Unauthorized => (
            reqwest::StatusCode::UNAUTHORIZED,
            Json(json!({"error": "invalid upstream key"})),
        )
            .into_response(),
        MockMode::RateLimited => (
            reqwest::StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "slow down"})),
        )
            .into_response(),
        MockMode::Success => Json(json!({
            "object": "list",
            "model": body.get("model").cloned().unwrap_or(Value::Null),
            "data": [{"object": "embedding", "index": 0, "embedding": [0.1, 0.2]}],
            "usage": {"prompt_tokens": 4, "total_tokens": 4}
        }))
        .into_response(),
    }
}

fn unique_email(prefix: &str) -> String {
    format!("{prefix}-{}@example.com", Uuid::new_v4())
}
