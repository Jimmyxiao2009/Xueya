//! Verruckt Cloud account/AI integration against a scripted loopback server.
//!
//! These tests exercise the real `CloudAccountClient` and `CloudAiProvider`
//! over HTTP: session restore, single-flight refresh + retry-once, signed-out
//! emission, error mapping, and Cloud AI entitlement/quota gating. No external
//! network is used.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use serde_json::json;
use tokio::sync::broadcast::error::RecvError;
use uuid::Uuid;
use verruckt_lib::ai::cloud::CloudAiProvider;
use verruckt_lib::ai::stream::{with_lifecycle, AiStreamEvent};
use verruckt_lib::ai::traits::{ChatMessage, ChatProvider, ChatRequest, EmbeddingProvider};
use verruckt_lib::cloud::client::{CloudAccountClient, REFRESH_TOKEN_REF};
use verruckt_lib::cloud::models::CloudSessionState;
use verruckt_lib::error::AppError;
use verruckt_lib::secrets::keyring_store::{InMemorySecretStore, SecretStore};
use verruckt_lib::state::{AppPaths, AppState};

#[derive(Clone)]
struct MockResponse {
    status: u16,
    content_type: &'static str,
    body: String,
}

impl MockResponse {
    fn json(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            content_type: "application/json",
            body: body.into(),
        }
    }
}

struct MockServer {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<String>>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl MockServer {
    fn start(responses: Vec<MockResponse>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        let addr = listener.local_addr().expect("local addr");
        let shutdown = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let queue = Arc::new(Mutex::new(VecDeque::from(responses)));

        let thread_shutdown = shutdown.clone();
        let thread_requests = requests.clone();
        let thread_queue = queue.clone();
        let handle = std::thread::spawn(move || {
            while !thread_shutdown.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let connection_requests = thread_requests.clone();
                        let connection_queue = thread_queue.clone();
                        std::thread::spawn(move || {
                            serve_one(stream, &connection_requests, &connection_queue);
                        });
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(3));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            addr,
            shutdown,
            requests,
            handle: Some(handle),
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().expect("requests lock").clone()
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.addr);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn read_request(stream: &mut TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                request.extend_from_slice(&buffer[..read]);
                if let Some(header_end) = find_subsequence(&request, b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            if name.eq_ignore_ascii_case("content-length") {
                                value.trim().parse::<usize>().ok()
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);
                    if request.len() >= header_end + 4 + content_length {
                        break;
                    }
                } else if request.len() > 2 * 1024 * 1024 {
                    break;
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break;
            }
            Err(_) => break,
        }
    }
    request
}

fn serve_one(
    mut stream: TcpStream,
    requests: &Arc<Mutex<Vec<String>>>,
    queue: &Arc<Mutex<VecDeque<MockResponse>>>,
) {
    let _ = stream.set_nonblocking(false);
    let request = read_request(&mut stream);
    let request_line = String::from_utf8_lossy(&request).into_owned();
    requests.lock().expect("requests lock").push(request_line);

    let response = queue
        .lock()
        .expect("queue lock")
        .pop_front()
        .unwrap_or_else(|| {
            MockResponse::json(
                500,
                r#"{"error":{"code":"internal_error","message":"no scripted response"}}"#,
            )
        });
    let reason = match response.status {
        200 => "OK",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        409 => "Conflict",
        413 => "Payload Too Large",
        _ => "Internal Server Error",
    };
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.status,
        reason,
        response.content_type,
        response.body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(response.body.as_bytes());
    let _ = stream.flush();
}

fn user_body() -> serde_json::Value {
    json!({
        "id": Uuid::from_u128(0x1234),
        "email": "e2e@example.com",
        "display_name": null,
        "created_at": "2026-01-01T00:00:00Z"
    })
}

fn auth_body(access: &str, refresh: &str) -> String {
    json!({
        "access_token": access,
        "refresh_token": refresh,
        "token_type": "Bearer",
        "expires_in": 3600,
        "user": user_body(),
        "device": {
            "id": Uuid::new_v4(),
            "name": "E2E device",
            "platform": "windows",
            "app_version": "0.1.0",
            "created_at": "2026-01-01T00:00:00Z",
            "last_seen_at": "2026-01-01T00:00:00Z"
        }
    })
    .to_string()
}

fn entitlements_body(official_ai: bool, used: i64, limit: i64) -> String {
    json!({
        "plan": "pro",
        "features": { "cloud_sync": true, "official_ai": official_ai },
        "limits": {
            "max_devices": 5,
            "max_cloud_storage_bytes": 5368709120_i64,
            "monthly_ai_credits": limit
        },
        "usage": {
            "devices_used": 1,
            "monthly_ai_credits_used": used,
            "cloud_storage_bytes_used": 0
        }
    })
    .to_string()
}

fn error_body(code: &str, message: &str) -> String {
    json!({ "error": { "code": code, "message": message } }).to_string()
}

fn chat_body(content: &str) -> String {
    json!({
        "choices": [{ "message": { "content": content } }],
        "model": "verruckt-fast",
        "usage": { "prompt_tokens": 3, "completion_tokens": 4 }
    })
    .to_string()
}

fn embedding_body(values: &str) -> String {
    format!(r#"{{"data":[{{"embedding":[{values}]}}],"model":"verruckt-embedding"}}"#)
}

async fn test_pool() -> (AppState, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let state = AppState::new_with_secrets(
        AppPaths::new(dir.path()),
        Arc::new(InMemorySecretStore::new()),
    )
    .await
    .expect("app state");
    (state, dir)
}

async fn logged_in_client_with_secrets(
    server: &MockServer,
) -> (Arc<CloudAccountClient>, Arc<dyn SecretStore>) {
    let (state, dir) = test_pool().await;
    let secrets: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let client = Arc::new(CloudAccountClient::new(
        reqwest::Client::new(),
        secrets.clone(),
        state.pool.clone(),
        server.base_url(),
    ));
    // Keep the temp dir alive for the duration of the returned client by
    // leaking it: tests are short-lived and the OS cleans temp files up.
    std::mem::forget(dir);
    client
        .login("e2e@example.com".to_owned(), "password123".to_owned())
        .await
        .expect("login");
    (client, secrets)
}

async fn logged_in_client(server: &MockServer) -> Arc<CloudAccountClient> {
    logged_in_client_with_secrets(server).await.0
}

#[tokio::test]
async fn registry_reports_cloud_unavailable_without_silent_fallback() {
    use verruckt_lib::ai::registry::ProviderRegistry;
    for (official, used, limit) in [(false, 0, 1000), (true, 10, 10)] {
        let server = MockServer::start(vec![
            MockResponse::json(200, auth_body("access-1", "refresh-1")),
            MockResponse::json(200, json!({ "user": user_body() }).to_string()),
            MockResponse::json(200, entitlements_body(official, used, limit)),
        ]);
        let client = logged_in_client(&server).await;
        let (state, _dir) = test_pool().await;
        sqlx::query(r#"INSERT INTO app_settings(key,value_json,updated_at) VALUES ('ai_provider_mode','"cloud"',?) ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json"#)
            .bind(chrono::Utc::now().to_rfc3339()).execute(&state.pool).await.unwrap();
        let registry = ProviderRegistry::new(state.pool.clone(), reqwest::Client::new(), Arc::new(InMemorySecretStore::new())).with_cloud(client);

        // Cloud mode must fail loudly instead of quietly becoming the
        // deterministic offline provider when entitlement or quota is missing.
        let error = registry
            .default_chat_provider()
            .await
            .err()
            .expect("cloud mode without entitlement must fail");
        assert!(error
            .to_string()
            .contains("Cloud AI is temporarily unavailable"));
        assert!(registry.is_mock_active().await.is_err());
    }
}

#[tokio::test]
async fn single_flight_refresh_retries_the_original_request_once() {
    let server = MockServer::start(vec![
        MockResponse::json(200, auth_body("access-1", "refresh-1")),
        MockResponse::json(200, json!({ "user": user_body() }).to_string()),
        MockResponse::json(200, entitlements_body(true, 0, 1000)),
        MockResponse::json(
            401,
            error_body("access_token_expired", "access token expired"),
        ),
        MockResponse::json(200, auth_body("access-2", "refresh-2")),
        MockResponse::json(200, "[]"),
    ]);
    let client = logged_in_client(&server).await;

    let devices = client.devices().await.expect("devices after refresh");
    assert!(devices.is_empty());

    let requests = server.requests();
    let refresh_count = requests
        .iter()
        .filter(|line| line.contains("/api/v1/auth/refresh"))
        .count();
    assert_eq!(refresh_count, 1, "{requests:?}");
    let device_count = requests
        .iter()
        .filter(|line| line.contains("/api/v1/me/devices"))
        .count();
    assert_eq!(
        device_count, 2,
        "original request should be retried once: {requests:?}"
    );
}

#[tokio::test]
async fn refresh_failure_clears_the_session_and_emits_signed_out() {
    let server = MockServer::start(vec![
        MockResponse::json(200, auth_body("access-1", "refresh-1")),
        MockResponse::json(200, json!({ "user": user_body() }).to_string()),
        MockResponse::json(200, entitlements_body(true, 0, 1000)),
        MockResponse::json(
            401,
            error_body("access_token_expired", "access token expired"),
        ),
        MockResponse::json(
            401,
            error_body("invalid_refresh_token", "refresh token is not valid"),
        ),
    ]);
    let (client, secrets) = logged_in_client_with_secrets(&server).await;
    let mut events = client.subscribe();

    let error = client.devices().await.expect_err("refresh must fail");
    assert!(matches!(error, AppError::CloudAuth(_)), "{error:?}");

    let mut signed_out = false;
    for _ in 0..8 {
        match tokio::time::timeout(Duration::from_secs(1), events.recv()).await {
            Ok(Ok(event)) if event.event == "cloud_signed_out" => {
                signed_out = true;
                break;
            }
            Ok(Ok(_)) => continue,
            Ok(Err(RecvError::Lagged(_))) => continue,
            Ok(Err(RecvError::Closed)) | Err(_) => break,
        }
    }
    assert!(signed_out, "expected a cloud_signed_out event");

    let state = client.account_state().await.expect("anonymous state");
    assert!(!state.logged_in);
    assert_eq!(state.state, CloudSessionState::SignedOut);
    assert!(
        !secrets
            .secret_exists(REFRESH_TOKEN_REF)
            .await
            .expect("secret lookup"),
        "an explicit auth failure must remove the refresh token"
    );
}

#[tokio::test]
async fn concurrent_force_refresh_only_hits_the_refresh_endpoint_once() {
    let server = MockServer::start(vec![
        MockResponse::json(200, auth_body("access-1", "refresh-1")),
        MockResponse::json(200, json!({ "user": user_body() }).to_string()),
        MockResponse::json(200, entitlements_body(true, 0, 1000)),
        MockResponse::json(200, auth_body("access-2", "refresh-2")),
    ]);
    let client = logged_in_client(&server).await;

    let (first, second) = tokio::join!(client.force_refresh(), client.force_refresh());
    first.expect("first refresh");
    second.expect("second refresh");

    let refresh_count = server
        .requests()
        .iter()
        .filter(|line| line.contains("/api/v1/auth/refresh"))
        .count();
    assert_eq!(refresh_count, 1);
}

#[tokio::test]
async fn eight_concurrent_requests_share_one_refresh() {
    let mut responses = vec![
        MockResponse::json(200, auth_body("access-1", "refresh-1")),
        MockResponse::json(200, json!({ "user": user_body() }).to_string()),
        MockResponse::json(200, entitlements_body(true, 0, 1000)),
    ];
    // Every request receives a 401 first, which is the realistic "access token
    // expired while eight requests were already in flight" case.
    for _ in 0..8 {
        responses.push(MockResponse::json(
            401,
            error_body("access_token_expired", "access token expired"),
        ));
    }
    responses.push(MockResponse::json(200, auth_body("access-2", "refresh-2")));
    for _ in 0..8 {
        responses.push(MockResponse::json(200, "[]"));
    }
    let server = MockServer::start(responses);
    let client = logged_in_client(&server).await;

    let mut handles = Vec::new();
    for _ in 0..8 {
        let client = client.clone();
        handles.push(tokio::spawn(async move { client.devices().await }));
    }
    for handle in handles {
        handle
            .await
            .expect("request task")
            .expect("device request after refresh");
    }

    let requests = server.requests();
    let refresh_count = requests
        .iter()
        .filter(|line| line.contains("/api/v1/auth/refresh"))
        .count();
    assert_eq!(
        refresh_count, 1,
        "refresh must be single-flight: {requests:?}"
    );
    let device_count = requests
        .iter()
        .filter(|line| line.contains("/api/v1/me/devices"))
        .count();
    assert_eq!(
        device_count, 16,
        "each of the eight requests should be retried exactly once: {requests:?}"
    );
}

#[tokio::test]
async fn cloud_streaming_relays_started_delta_delta_completed() {
    let sse_body = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\" world\"},\"finish_reason\":null}]}\n\n",
        "data: [DONE]\n\n",
    );
    let server = MockServer::start(vec![
        MockResponse::json(200, auth_body("access-1", "refresh-1")),
        MockResponse::json(200, json!({ "user": user_body() }).to_string()),
        MockResponse::json(200, entitlements_body(true, 0, 1000)),
        MockResponse {
            status: 200,
            content_type: "text/event-stream",
            body: sse_body.to_owned(),
        },
    ]);
    let client = logged_in_client(&server).await;
    let provider = CloudAiProvider::new(
        client,
        reqwest::Client::new(),
        "verruckt-fast",
        "verruckt-embedding",
    );
    let stream = provider
        .chat_stream(ChatRequest::new(
            "verruckt-fast",
            vec![ChatMessage::user("hi")],
        ))
        .await
        .expect("cloud stream");
    let mut events = with_lifecycle(stream, "verruckt-fast");

    let mut collected = Vec::new();
    while let Some(event) = events.next().await {
        collected.push(event.expect("stream event"));
    }

    assert_eq!(collected.len(), 4, "{collected:?}");
    assert!(matches!(collected[0], AiStreamEvent::Started { .. }));
    assert!(
        matches!(&collected[1], AiStreamEvent::Delta { text } if text == "Hello"),
        "{collected:?}"
    );
    assert!(
        matches!(&collected[2], AiStreamEvent::Delta { text } if text == " world"),
        "{collected:?}"
    );
    assert!(matches!(collected[3], AiStreamEvent::Completed { .. }));
}

#[tokio::test]
async fn malformed_sse_event_becomes_a_terminal_failure() {
    let server = MockServer::start(vec![
        MockResponse::json(200, auth_body("access-1", "refresh-1")),
        MockResponse::json(200, json!({ "user": user_body() }).to_string()),
        MockResponse::json(200, entitlements_body(true, 0, 1000)),
        MockResponse {
            status: 200,
            content_type: "text/event-stream",
            body: "data: this-is-not-json\n\n".to_owned(),
        },
    ]);
    let client = logged_in_client(&server).await;
    let provider = CloudAiProvider::new(
        client,
        reqwest::Client::new(),
        "verruckt-fast",
        "verruckt-embedding",
    );
    let stream = provider
        .chat_stream(ChatRequest::new(
            "verruckt-fast",
            vec![ChatMessage::user("hi")],
        ))
        .await
        .expect("cloud stream");
    let mut events = with_lifecycle(stream, "verruckt-fast");

    let mut collected = Vec::new();
    while let Some(event) = events.next().await {
        collected.push(event.expect("stream event"));
    }

    assert!(matches!(collected[0], AiStreamEvent::Started { .. }));
    assert!(
        collected
            .iter()
            .any(|event| matches!(event, AiStreamEvent::Failed { .. })),
        "malformed SSE must terminate with Failed: {collected:?}"
    );
    assert!(
        !collected
            .iter()
            .any(|event| matches!(event, AiStreamEvent::Completed { .. })),
        "malformed SSE must not report Completed: {collected:?}"
    );
}

#[tokio::test]
async fn cloud_streaming_upstream_error_does_not_hang() {
    let server = MockServer::start(vec![
        MockResponse::json(200, auth_body("access-1", "refresh-1")),
        MockResponse::json(200, json!({ "user": user_body() }).to_string()),
        MockResponse::json(200, entitlements_body(true, 0, 1000)),
        MockResponse::json(500, error_body("upstream_error", "upstream returned 500")),
    ]);
    let client = logged_in_client(&server).await;
    let provider = CloudAiProvider::new(
        client,
        reqwest::Client::new(),
        "verruckt-fast",
        "verruckt-embedding",
    );
    let error = match provider
        .chat_stream(ChatRequest::new(
            "verruckt-fast",
            vec![ChatMessage::user("hi")],
        ))
        .await
    {
        Ok(_) => panic!("upstream error must be returned"),
        Err(error) => error,
    };
    assert!(matches!(error, AppError::CloudUnavailable(_)), "{error:?}");
}

#[tokio::test]
async fn cloud_stream_can_be_cancelled_after_first_delta() {
    let sse_body = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"first\"},\"finish_reason\":null}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"second\"},\"finish_reason\":null}]}\n\n",
        "data: [DONE]\n\n",
    );
    let server = MockServer::start(vec![
        MockResponse::json(200, auth_body("access-1", "refresh-1")),
        MockResponse::json(200, json!({ "user": user_body() }).to_string()),
        MockResponse::json(200, entitlements_body(true, 0, 1000)),
        MockResponse {
            status: 200,
            content_type: "text/event-stream",
            body: sse_body.to_owned(),
        },
    ]);
    let client = logged_in_client(&server).await;
    let provider = CloudAiProvider::new(
        client,
        reqwest::Client::new(),
        "verruckt-fast",
        "verruckt-embedding",
    );
    let mut stream = provider
        .chat_stream(ChatRequest::new(
            "verruckt-fast",
            vec![ChatMessage::user("hi")],
        ))
        .await
        .expect("cloud stream");
    let first = stream.next().await.expect("first item").expect("delta");
    assert_eq!(first.content, "first");
    // Dropping the stream is the client cancellation path. The test passes if
    // no panic, blocking, or background job remains.
    drop(stream);
}

#[tokio::test]
async fn cloud_error_codes_map_to_typed_errors() {
    let server = MockServer::start(vec![
        MockResponse::json(200, auth_body("access-1", "refresh-1")),
        MockResponse::json(200, json!({ "user": user_body() }).to_string()),
        MockResponse::json(200, entitlements_body(true, 0, 1000)),
        MockResponse::json(402, error_body("ai_quota_exceeded", "quota")),
        MockResponse::json(403, error_body("entitlement_required", "plan")),
        MockResponse::json(413, error_body("storage_quota_exceeded", "storage")),
        MockResponse::json(409, error_body("sync_conflict", "revision")),
    ]);
    let client = logged_in_client(&server).await;

    let quota = client.ai_usage().await.expect_err("quota error");
    assert!(matches!(quota, AppError::CloudQuotaExceeded), "{quota:?}");
    let entitlement = client.ai_usage().await.expect_err("entitlement error");
    assert!(
        matches!(entitlement, AppError::CloudEntitlementRequired),
        "{entitlement:?}"
    );
    let storage = client.ai_usage().await.expect_err("storage error");
    assert!(
        matches!(storage, AppError::CloudStorageQuotaExceeded),
        "{storage:?}"
    );
    let conflict = client.ai_usage().await.expect_err("conflict error");
    assert!(
        matches!(conflict, AppError::CloudSyncConflict(_)),
        "{conflict:?}"
    );
}

#[tokio::test]
async fn cloud_ai_provider_uses_chat_completions_and_embeddings() {
    let server = MockServer::start(vec![
        MockResponse::json(200, auth_body("access-1", "refresh-1")),
        MockResponse::json(200, json!({ "user": user_body() }).to_string()),
        MockResponse::json(200, entitlements_body(true, 0, 1000)),
        MockResponse::json(200, chat_body("hello from cloud")),
        MockResponse::json(200, embedding_body("0.1,0.2,0.3")),
    ]);
    let client = logged_in_client(&server).await;
    let provider = CloudAiProvider::new(
        client,
        reqwest::Client::new(),
        "verruckt-fast",
        "verruckt-embedding",
    );

    let chat = provider
        .chat(ChatRequest::new(
            "verruckt-fast",
            vec![ChatMessage::user("hi")],
        ))
        .await
        .expect("cloud chat");
    assert_eq!(chat.content, "hello from cloud");

    let embeddings = provider
        .embed(vec!["alpha".to_owned()])
        .await
        .expect("cloud embeddings");
    assert_eq!(embeddings, vec![vec![0.1, 0.2, 0.3]]);

    let requests = server.requests();
    assert!(
        requests
            .iter()
            .any(|line| line.contains("POST /v1/chat/completions")),
        "{requests:?}"
    );
    assert!(
        requests
            .iter()
            .any(|line| line.contains("POST /v1/embeddings")),
        "{requests:?}"
    );
}

#[tokio::test]
async fn cloud_ai_provider_gates_entitlement_and_quota_without_calling_upstream() {
    let server = MockServer::start(vec![
        MockResponse::json(200, auth_body("access-1", "refresh-1")),
        MockResponse::json(200, json!({ "user": user_body() }).to_string()),
        MockResponse::json(200, entitlements_body(false, 0, 1000)),
    ]);
    let client = logged_in_client(&server).await;
    let provider = CloudAiProvider::new(
        client,
        reqwest::Client::new(),
        "verruckt-fast",
        "verruckt-embedding",
    );
    let error = provider
        .chat(ChatRequest::new(
            "verruckt-fast",
            vec![ChatMessage::user("hi")],
        ))
        .await
        .expect_err("official_ai is disabled");
    assert!(
        matches!(error, AppError::CloudEntitlementRequired),
        "{error:?}"
    );
    assert!(
        !server
            .requests()
            .iter()
            .any(|line| line.contains("/v1/chat/completions")),
        "gate should reject before the upstream call"
    );

    let server = MockServer::start(vec![
        MockResponse::json(200, auth_body("access-1", "refresh-1")),
        MockResponse::json(200, json!({ "user": user_body() }).to_string()),
        MockResponse::json(200, entitlements_body(true, 10, 10)),
    ]);
    let client = logged_in_client(&server).await;
    let provider = CloudAiProvider::new(
        client,
        reqwest::Client::new(),
        "verruckt-fast",
        "verruckt-embedding",
    );
    let error = provider
        .embed(vec!["alpha".to_owned()])
        .await
        .expect_err("quota is exhausted");
    assert!(matches!(error, AppError::CloudQuotaExceeded), "{error:?}");
    assert!(
        !server
            .requests()
            .iter()
            .any(|line| line.contains("/v1/embeddings")),
        "gate should reject before the upstream call"
    );
}

#[tokio::test]
async fn transient_restore_failure_keeps_the_refresh_token() {
    let server = MockServer::start(vec![
        MockResponse::json(200, auth_body("access-1", "refresh-1")),
        MockResponse::json(200, json!({ "user": user_body() }).to_string()),
        MockResponse::json(200, entitlements_body(true, 0, 1000)),
        // A fresh client restores from the keyring, refreshes successfully,
        // then hits a transient upstream failure while loading `/me`.
        MockResponse::json(200, auth_body("access-2", "refresh-2")),
        MockResponse::json(
            503,
            error_body("upstream_unavailable", "upstream is restarting"),
        ),
    ]);
    let (state, dir) = test_pool().await;
    let secrets: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());

    let first = CloudAccountClient::new(
        reqwest::Client::new(),
        secrets.clone(),
        state.pool.clone(),
        server.base_url(),
    );
    first
        .login("e2e@example.com".to_owned(), "password123".to_owned())
        .await
        .expect("login");

    let restarted = CloudAccountClient::new(
        reqwest::Client::new(),
        secrets.clone(),
        state.pool.clone(),
        server.base_url(),
    );
    let account = restarted.account_state().await.expect("account state");
    assert!(!account.logged_in);
    assert_eq!(account.state, CloudSessionState::DegradedOffline);
    assert_eq!(account.email.as_deref(), Some("e2e@example.com"));
    assert_eq!(account.plan.as_deref(), Some("pro"));
    assert!(
        account
            .entitlements
            .as_ref()
            .map(|value| value.official_ai)
            .unwrap_or(false),
        "degraded state should keep the last known entitlement snapshot"
    );
    assert!(
        secrets
            .secret_exists(REFRESH_TOKEN_REF)
            .await
            .expect("secret lookup"),
        "a transient failure must not delete the refresh token"
    );
    std::mem::forget(dir);
}

#[allow(dead_code)]
fn _assert_secret_ref_is_public() {
    // Keeps the REFRESH_TOKEN_REF import meaningful if the test body changes.
    let _ = REFRESH_TOKEN_REF;
}

#[tokio::test]
async fn logout_authenticates_revocation_and_resets_sync_session() {
    let server = MockServer::start(vec![
        MockResponse::json(200, auth_body("access-a", "refresh-a")),
        MockResponse::json(200, json!({"user":user_body()}).to_string()),
        MockResponse::json(200, entitlements_body(true, 0, 10000)),
        MockResponse::json(200, "{}"),
    ]);
    let (s, _dir) = test_pool().await;
    let secrets: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let client = CloudAccountClient::new(
        reqwest::Client::new(),
        secrets.clone(),
        s.pool.clone(),
        server.base_url(),
    );
    client
        .login("a@example.com".into(), "password123".into())
        .await
        .unwrap();
    assert!(client.device_id().await.unwrap().is_some());
    sqlx::query("UPDATE sync_state SET account_id='account-A',cursor='A-cursor',bootstrap_done=1 WHERE id=1").execute(&s.pool).await.unwrap();
    client.logout().await.unwrap();
    let requests = server.requests();
    let logout = requests
        .iter()
        .find(|r| r.contains("/auth/logout"))
        .unwrap();
    assert!(logout
        .to_ascii_lowercase()
        .contains("authorization: bearer access-a"));
    assert!(client.device_id().await.unwrap().is_none());
    let cursor: String = sqlx::query_scalar("SELECT cursor FROM sync_state WHERE id=1")
        .fetch_one(&s.pool)
        .await
        .unwrap();
    assert_eq!(cursor, "");
    assert!(!secrets.secret_exists(REFRESH_TOKEN_REF).await.unwrap());
    let owner: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE key='cloud_profile_owner'")
            .fetch_optional(&s.pool)
            .await
            .unwrap();
    assert!(owner.is_some(), "profile ownership survives sign out");
}

#[tokio::test]
async fn switching_account_cannot_reuse_the_previous_profiles_sync_data() {
    let mut other_auth: serde_json::Value =
        serde_json::from_str(&auth_body("other-access", "other-refresh")).unwrap();
    other_auth["user"]["id"] = json!(Uuid::from_u128(0x5678));
    other_auth["user"]["email"] = json!("other@example.com");
    let server = MockServer::start(vec![
        MockResponse::json(200, auth_body("access-a", "refresh-a")),
        MockResponse::json(200, json!({"user": user_body()}).to_string()),
        MockResponse::json(200, entitlements_body(true, 0, 10000)),
        MockResponse::json(200, "{}"),
        MockResponse::json(200, other_auth.to_string()),
    ]);
    let (state, _dir) = test_pool().await;
    let secrets: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let client = CloudAccountClient::new(
        reqwest::Client::new(),
        secrets.clone(),
        state.pool.clone(),
        server.base_url(),
    );
    client
        .login("e2e@example.com".into(), "password123".into())
        .await
        .unwrap();
    client.logout().await.unwrap();
    assert!(matches!(
        client
            .login("other@example.com".into(), "password123".into())
            .await,
        Err(AppError::CloudAuth(_))
    ));
    assert!(!secrets.secret_exists(REFRESH_TOKEN_REF).await.unwrap());
    assert!(client.device_id().await.unwrap().is_none());
    assert!(!client.account_state().await.unwrap().logged_in);
}

#[tokio::test]
async fn first_eligible_login_enables_sync_and_explicit_disable_is_respected() {
    let server = MockServer::start(vec![
        MockResponse::json(200, auth_body("access-1", "refresh-1")),
        MockResponse::json(200, json!({ "user": user_body() }).to_string()),
        MockResponse::json(200, entitlements_body(true, 0, 10000)),
        MockResponse::json(200, auth_body("access-2", "refresh-2")),
    ]);
    let (state, _dir) = test_pool().await;
    let secrets: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let client = CloudAccountClient::new(
        reqwest::Client::new(),
        secrets,
        state.pool.clone(),
        server.base_url(),
    );

    let account = client
        .login("e2e@example.com".into(), "password123".into())
        .await
        .expect("first login");
    assert!(
        account.sync_enabled,
        "first eligible login must enable cloud sync by default"
    );
    assert!(client.sync_enabled().await.unwrap());

    client
        .set_sync_enabled(false)
        .await
        .expect("explicit disable");
    let account = client
        .login("e2e@example.com".into(), "password123".into())
        .await
        .expect("second login");
    assert!(
        !account.sync_enabled,
        "an explicit sync disable must survive a later login"
    );
    assert!(!client.sync_enabled().await.unwrap());
}

