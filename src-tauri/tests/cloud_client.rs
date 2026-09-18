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

use serde_json::json;
use tokio::sync::broadcast::error::RecvError;
use uuid::Uuid;
use verruckt_lib::ai::cloud::CloudAiProvider;
use verruckt_lib::ai::traits::{ChatMessage, ChatProvider, ChatRequest, EmbeddingProvider};
use verruckt_lib::cloud::client::{CloudAccountClient, REFRESH_TOKEN_REF};
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
    let request_line = String::from_utf8_lossy(&request)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned();
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
        "id": Uuid::new_v4(),
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

async fn logged_in_client(server: &MockServer) -> Arc<CloudAccountClient> {
    let (state, dir) = test_pool().await;
    let secrets: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let client = Arc::new(CloudAccountClient::new(
        reqwest::Client::new(),
        secrets,
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
    client
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
    let client = logged_in_client(&server).await;
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

#[allow(dead_code)]
fn _assert_secret_ref_is_public() {
    // Keeps the REFRESH_TOKEN_REF import meaningful if the test body changes.
    let _ = REFRESH_TOKEN_REF;
}
