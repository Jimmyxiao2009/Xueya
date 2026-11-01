//! Provider compatibility matrix against a tiny scripted HTTP server.
//!
//! These tests exercise the real `OpenAiCompatibleProvider` over loopback: no
//! API keys, no external network, and every response is deterministic. They are
//! the guard against a provider gateway that is "OpenAI-compatible" on paper
//! but differs in status codes, streaming envelopes or embedding payloads.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use verruckt_lib::ai::http::RetryPolicy;
use verruckt_lib::ai::openai_compatible::OpenAiCompatibleProvider;
use verruckt_lib::ai::traits::{ChatMessage, ChatProvider, ChatRequest, EmbeddingProvider};
use verruckt_lib::domain::ProviderProtocol;

#[tokio::test]
async fn topic_only_course_uses_real_provider_and_template_lesson_keeps_its_origin() {
    use verruckt_lib::{ai::registry::ProviderUpsert, secrets::keyring_store::InMemorySecretStore,
        services::course_service::{CourseService, RouteRequest}, state::{AppState, AppPaths}};
    let plan = serde_json::json!({"title":"Model-only astronomy", "description":"Generated over HTTP", "learningObjectives":["orbits"], "prerequisites":[],
        "nodes":[{"id":"orbits","title":"Orbital mechanics","objective":"Explain orbit","description":"Orbit","concepts":["gravity"],"dependencies":[],"evidenceChunkIds":[],"estimatedMinutes":10}]});
    let response = serde_json::json!({"choices":[{"message":{"content":plan.to_string()}}]});
    // The outline remains visible when its independent content worker fails.
    let server = MockServer::start(vec![MockResponse::json(200, response.to_string()), MockResponse::json(401, "{}"), MockResponse::json(401, "{}")]);
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new_with_secrets(AppPaths::new(dir.path()), Arc::new(InMemorySecretStore::new())).await.unwrap();
    assert!(state.providers.is_mock_active().await.unwrap());
    state.providers.upsert(ProviderUpsert { id:None, display_name:"Test BYOK".into(), base_url:server.base_url(), chat_model:"test".into(), embedding_model:"test".into(), api_key:Some("test-key".into()), protocol:ProviderProtocol::ChatCompletions, enabled:true }).await.unwrap();
    sqlx::query("INSERT INTO app_settings (key, value_json, updated_at) VALUES ('ai_provider_mode', ?, ?) ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json")
        .bind(serde_json::json!("byok").to_string())
        .bind("2026-01-01T00:00:00Z")
        .execute(&state.pool)
        .await
        .unwrap();

    assert!(!state.providers.is_mock_active().await.unwrap());
    let course = CourseService::new(&state).create_course(RouteRequest::new("astronomy"), None).await.unwrap();
    assert_eq!(course.title, "Model-only astronomy");
    let metadata: serde_json::Value = serde_json::from_str(&course.metadata_json).unwrap();
    assert_eq!(metadata["origin"], "byok_general");
    let nodes = verruckt_lib::db::repo::courses::list_nodes(&state.pool, course.id).await.unwrap();
    let node_meta: serde_json::Value = serde_json::from_str(&nodes[0].metadata_json).unwrap();
    assert_eq!(node_meta["quizOrigin"], "pending");
    assert!(CourseService::new(&state).prepare_node(nodes[0].id, None).await.is_err());
    assert!(nodes[0].lesson_cache_json.is_none());
    assert!(server.requests().iter().any(|request| request.contains("chat/completions")));
}

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

    fn sse(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            content_type: "text/event-stream",
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

/// Drain the request (headers + declared body) before replying. A server that
/// closes while the client is still writing turns a clean HTTP response into a
/// connection reset, which the retry policy would then classify as transient.
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
    // `TcpListener::set_nonblocking(true)` can leak the non-blocking flag onto
    // accepted sockets on some platforms. Force each connection back to
    // blocking mode before applying the read timeout, otherwise a scheduled
    // request can look like an immediate `WouldBlock` and be dropped under
    // parallel test load.
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let request = read_request(&mut stream);
    if request.is_empty() {
        return;
    }
    let request_text = String::from_utf8_lossy(&request);
    let first_line = request_text.lines().next().unwrap_or_default().to_owned();
    requests.lock().expect("requests lock").push(first_line);

    let response = queue
        .lock()
        .expect("queue lock")
        .pop_front()
        .unwrap_or_else(|| MockResponse::json(500, r#"{"error":"no scripted response"}"#));
    let reason = match response.status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        _ => "Mock",
    };
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.status,
        reason,
        response.content_type,
        response.body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(response.body.as_bytes());
    let _ = stream.flush();
}

fn provider(server: &MockServer, protocol: ProviderProtocol) -> OpenAiCompatibleProvider {
    // No idle connection reuse: the mock server is intentionally small, and a
    // stale keep-alive socket under load must not look like a transient
    // provider network failure.
    let client = reqwest::Client::builder()
        .pool_max_idle_per_host(0)
        // The developer machine may export HTTP_PROXY/HTTPS_PROXY for real
        // providers; loopback tests must never be routed through it.
        .no_proxy()
        .build()
        .expect("test http client");
    OpenAiCompatibleProvider::with_protocol(
        client,
        server.base_url(),
        "test-key-not-real",
        "chat-model",
        "embed-model",
        protocol,
    )
    .with_retry(RetryPolicy {
        max_retries: 1,
        base_delay: Duration::from_millis(1),
    })
}

fn chat_body(content: &str) -> String {
    format!(
        r#"{{"choices":[{{"message":{{"content":"{content}"}}}}],"model":"chat-model","usage":{{"prompt_tokens":3,"completion_tokens":4}}}}"#
    )
}

fn embedding_body(values: &str) -> String {
    format!(r#"{{"data":[{{"embedding":[{values}]}}],"model":"embed-model"}}"#)
}

#[tokio::test]
async fn chat_completions_and_embeddings_use_their_openai_paths() {
    let server = MockServer::start(vec![
        MockResponse::json(200, chat_body("hello")),
        MockResponse::json(200, embedding_body("0.1,0.2,0.3")),
    ]);
    let provider = provider(&server, ProviderProtocol::ChatCompletions);

    let chat = provider
        .chat(ChatRequest::new(
            "chat-model",
            vec![ChatMessage::user("hi")],
        ))
        .await
        .expect("chat");
    assert_eq!(chat.content, "hello");
    assert_eq!(chat.usage.expect("usage").completion_tokens, Some(4));

    let embeddings = provider
        .embed(vec!["alpha".to_owned()])
        .await
        .expect("embedding");
    assert_eq!(embeddings.len(), 1);
    assert_eq!(embeddings[0], vec![0.1, 0.2, 0.3]);

    let requests = server.requests();
    assert!(requests[0].contains("POST /v1/chat/completions HTTP/1.1"));
    assert!(requests[1].contains("POST /v1/embeddings HTTP/1.1"));
}

#[tokio::test]
async fn responses_protocol_uses_the_responses_endpoint() {
    let server = MockServer::start(vec![MockResponse::json(
        200,
        r#"{"output":[{"content":[{"type":"output_text","text":"from responses"}]}],"model":"chat-model","usage":{"input_tokens":1,"output_tokens":2,"total_tokens":3}}"#,
    )]);
    let provider = provider(&server, ProviderProtocol::Responses);

    let chat = provider
        .chat(ChatRequest::new(
            "chat-model",
            vec![ChatMessage::system("be terse"), ChatMessage::user("hello")],
        ))
        .await
        .expect("responses chat");
    assert_eq!(chat.content, "from responses");
    assert!(server.requests()[0].contains("POST /v1/responses HTTP/1.1"));
}

#[tokio::test]
async fn retryable_statuses_are_retried_then_succeed() {
    let server = MockServer::start(vec![
        MockResponse::json(429, r#"{"error":"slow down"}"#),
        MockResponse::json(200, chat_body("after retry")),
    ]);
    let provider = provider(&server, ProviderProtocol::ChatCompletions);

    let chat = provider
        .chat(ChatRequest::new(
            "chat-model",
            vec![ChatMessage::user("hi")],
        ))
        .await
        .expect("retried chat");
    assert_eq!(chat.content, "after retry");
    assert_eq!(server.requests().len(), 2);
}

#[tokio::test]
async fn authentication_errors_are_not_retried() {
    let server = MockServer::start(vec![MockResponse::json(
        401,
        r#"{"error":{"message":"bad key"}}"#,
    )]);
    let provider = provider(&server, ProviderProtocol::ChatCompletions);

    let error = provider
        .chat(ChatRequest::new(
            "chat-model",
            vec![ChatMessage::user("hi")],
        ))
        .await
        .expect_err("401 must fail");
    assert_eq!(error.code(), "PROVIDER_AUTH_ERROR");
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn malformed_json_is_reported_as_a_provider_error() {
    let server = MockServer::start(vec![MockResponse::json(200, "{not json")]);
    let provider = provider(&server, ProviderProtocol::ChatCompletions);

    let error = provider
        .chat(ChatRequest::new(
            "chat-model",
            vec![ChatMessage::user("hi")],
        ))
        .await
        .expect_err("malformed json must fail");
    assert_eq!(error.code(), "AI_RESPONSE_INVALID");
}

#[tokio::test]
async fn sse_stream_yields_incremental_deltas() {
    let server = MockServer::start(vec![MockResponse::sse(
        "data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\n\
         data: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n\n\
         data: [DONE]\n\n",
    )]);
    let provider = provider(&server, ProviderProtocol::ChatCompletions);

    let mut stream = provider
        .chat_stream(ChatRequest::new(
            "chat-model",
            vec![ChatMessage::user("hi")],
        ))
        .await
        .expect("stream");
    let mut text = String::new();
    while let Some(item) = stream.next().await {
        text.push_str(&item.expect("delta").content);
    }
    assert_eq!(text, "Hello");
}

#[tokio::test]
async fn empty_embeddings_are_rejected_before_they_reach_the_index() {
    let server = MockServer::start(vec![MockResponse::json(200, r#"{"data":[]}"#)]);
    let provider = provider(&server, ProviderProtocol::ChatCompletions);

    let error = provider
        .embed(vec!["alpha".to_owned()])
        .await
        .expect_err("empty embedding payload must fail");
    assert_eq!(error.code(), "EMBEDDING_ERROR");
}

#[tokio::test]
async fn auto_probe_reports_protocol_capabilities_and_dimension() {
    let server = MockServer::start(vec![
        // structured chat-completions probe
        MockResponse::json(200, chat_body("ok")),
        // responses probe
        MockResponse::json(
            200,
            r#"{"output":[{"content":[{"type":"output_text","text":"ok"}]}]}"#,
        ),
        // embeddings probe
        MockResponse::json(200, embedding_body("0.5,0.25,0.125")),
        // streaming probe
        MockResponse::sse("data: [DONE]\n\n"),
    ]);
    let provider = provider(&server, ProviderProtocol::Auto);

    let capabilities = provider.probe_capabilities().await;
    assert!(capabilities.chat_completions, "{capabilities:?}");
    assert!(capabilities.responses, "{capabilities:?}");
    assert!(capabilities.embeddings, "{capabilities:?}");
    assert!(capabilities.streaming, "{capabilities:?}");
    assert_eq!(capabilities.embedding_dimension, Some(3));
    assert_eq!(capabilities.protocol, Some(ProviderProtocol::Responses));
    assert!(capabilities.probe_errors.is_empty(), "{capabilities:?}");
}

#[tokio::test]
async fn byok_mode_without_provider_never_falls_back_to_mock() {
    use verruckt_lib::{secrets::keyring_store::InMemorySecretStore, state::{AppState, AppPaths}};
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new_with_secrets(
        AppPaths::new(dir.path()),
        Arc::new(InMemorySecretStore::new()),
    )
    .await
    .unwrap();

    // A fresh install defaults to explicit local-only mode, where deterministic
    // output is allowed and labelled as mock.
    assert!(state.providers.is_mock_active().await.unwrap());

    // Selecting BYOK without configuring a provider is a real configuration
    // error, not a silent switch back to deterministic output.
    sqlx::query(
        "INSERT INTO app_settings (key, value_json, updated_at) VALUES ('ai_provider_mode', ?, ?) \
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
    )
    .bind(serde_json::json!("byok").to_string())
    .bind("2026-01-01T00:00:00Z")
    .execute(&state.pool)
    .await
    .unwrap();

    let error = state
        .providers
        .default_chat_provider()
        .await
        .err()
        .expect("BYOK without a provider must fail");
    assert!(matches!(error, verruckt_lib::error::AppError::InvalidInput(_)));
    assert!(error.to_string().contains("no provider"));
}