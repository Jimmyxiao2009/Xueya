#![cfg(feature = "web-server")]
use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;
use verruckt_lib::{
    ai::registry::ProviderUpsert,
    db::repo::{courses, learning_loop},
    domain::ProviderProtocol,
    secrets::keyring_store::InMemorySecretStore,
    services::{
        course_service::{CourseService, RouteRequest},
        node_agent,
    },
    state::{AppPaths, AppState},
};

#[derive(Clone, Default)]
struct Upstream {
    active: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
    released: Arc<AtomicBool>,
    fail: Arc<AtomicBool>,
}

async fn chat(State(s): State<Upstream>, Json(body): Json<Value>) -> (StatusCode, Json<Value>) {
    let system = body["messages"][0]["content"].as_str().unwrap();
    let content = if system.contains("content agent") {
        let context: Value =
            serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
        assert_eq!(context["outline"].as_array().unwrap().len(), 6);
        let title = context["assignedNode"]["title"].as_str().unwrap();
        s.calls.fetch_add(1, Ordering::SeqCst);
        let active = s.active.fetch_add(1, Ordering::SeqCst) + 1;
        s.peak.fetch_max(active, Ordering::SeqCst);
        while !s.released.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        tokio::time::sleep(Duration::from_millis(80)).await;
        s.active.fetch_sub(1, Ordering::SeqCst);
        if title == "Node 2" && s.fail.load(Ordering::SeqCst) {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error":{"message":"test failure"}})),
            );
        }
        json!({"lesson":{"title":title,"explanation":format!("Detailed worked example for {title}"),"keyPoints":["concept"],"diagnosticQuestion":"Explain why", "citations":[]},
            "quiz":{"questions":[{"kind":"single","prompt":"Which?","options":["yes","no"],"answers":["yes"],"explanation":"Because","difficulty":0.4}]}})
    } else {
        json!({"title":"Agent outline", "description":"Test", "learningObjectives":["Learn"], "prerequisites":[], "nodes":(0..6).map(|i| json!({"id":format!("n{i}"),"title":format!("Node {i}"),"objective":"Learn", "description":"Test", "concepts":["concept"],"dependencies":if i==0 {vec![]} else {vec![format!("n{}",i-1)]},"evidenceChunkIds":[],"estimatedMinutes":10})).collect::<Vec<_>>()})
    };
    (
        StatusCode::OK,
        Json(
            json!({"choices":[{"message":{"content":content.to_string()},"finish_reason":"stop"}]}),
        ),
    )
}

#[tokio::test]
async fn outline_returns_before_workers_and_workers_are_parallel_atomic_and_retryable() {
    let upstream = Upstream::default();
    upstream.fail.store(true, Ordering::SeqCst);
    let app = Router::new()
        .route("/v1/chat/completions", post(chat))
        .with_state(upstream.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new_with_secrets(
        AppPaths::new(dir.path()),
        Arc::new(InMemorySecretStore::new()),
    )
    .await
    .unwrap();
    state
        .providers
        .upsert(ProviderUpsert {
            id: None,
            display_name: "Agent test".into(),
            base_url: format!("http://{address}"),
            chat_model: "step-5-preview".into(),
            embedding_model: "unused".into(),
            api_key: Some("test-only".into()),
            enabled: true,
            protocol: ProviderProtocol::ChatCompletions,
        })
        .await
        .unwrap();
    let service = CourseService::new(&state);
    let course = tokio::time::timeout(
        Duration::from_secs(5),
        service.create_course(RouteRequest::new("Test"), None),
    )
    .await
    .expect("outline must not wait for blocked workers")
    .unwrap();
    let nodes = courses::list_nodes(&state.pool, course.id).await.unwrap();
    assert_eq!(nodes.len(), 6);
    assert!(nodes.iter().all(|n| n.lesson_cache_json.is_none()));
    tokio::time::timeout(Duration::from_secs(5), async {
        while upstream.active.load(Ordering::SeqCst) < 4 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(upstream.peak.load(Ordering::SeqCst), 4);
    upstream.released.store(true, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let jobs = state.jobs.list().await.unwrap();
            if jobs.len() == 6
                && jobs.iter().all(|j| {
                    matches!(
                        j.status,
                        verruckt_lib::domain::JobStatus::Completed
                            | verruckt_lib::domain::JobStatus::Failed
                    )
                })
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let nodes = courses::list_nodes(&state.pool, course.id).await.unwrap();
    assert_eq!(
        nodes
            .iter()
            .filter(|n| n.lesson_cache_json.is_some())
            .count(),
        5
    );
    assert!(nodes[2].lesson_cache_json.is_none());
    assert!(learning_loop::list_quiz_questions(&state.pool, nodes[2].id)
        .await
        .unwrap()
        .is_empty());
    let calls = upstream.calls.load(Ordering::SeqCst);
    upstream.fail.store(false, Ordering::SeqCst);
    // Two opens for one failed node must generate once, then both read the cache.
    let (a, b) = tokio::join!(
        service.prepare_node(nodes[2].id, None),
        service.prepare_node(nodes[2].id, None)
    );
    assert_eq!(a.unwrap().questions.len(), 1);
    assert_eq!(b.unwrap().generated_by, "byok_general");
    assert_eq!(upstream.calls.load(Ordering::SeqCst), calls + 1);
    let ready = courses::get_node(&state.pool, nodes[2].id).await.unwrap();
    let meta: Value = serde_json::from_str(&ready.metadata_json).unwrap();
    assert_eq!(meta["contentStatus"], "ready");
    assert_eq!(meta["contentModel"], "step-5-preview");
    // Restart scheduling excludes completed nodes, preserving quiz IDs/attempts.
    node_agent::resume(&state).await.unwrap();
    assert_eq!(upstream.calls.load(Ordering::SeqCst), calls + 1);

    // Simulate a process interruption between claiming and completing a node.
    sqlx::query("UPDATE course_nodes SET metadata_json = json_set(metadata_json, '$.contentStatus', 'generating'), lesson_cache_json = NULL WHERE id = ?")
        .bind(nodes[0].id).execute(&state.pool).await.unwrap();
    node_agent::resume(&state).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let node = courses::get_node(&state.pool, nodes[0].id).await.unwrap();
            if node.lesson_cache_json.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(upstream.calls.load(Ordering::SeqCst), calls + 2);
    let untouched = courses::get_node(&state.pool, nodes[2].id).await.unwrap();
    assert_eq!(untouched.lesson_cache_json, ready.lesson_cache_json);

    // Deleting a published outline while its content requests are in flight
    // must not recreate any course, quiz, or lesson rows on completion.
    upstream.released.store(false, Ordering::SeqCst);
    let deleted = service
        .create_course(RouteRequest::new("Delete in flight"), None)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while upstream.active.load(Ordering::SeqCst) < 4 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    courses::delete_course(&state.pool, deleted.id)
        .await
        .unwrap();
    upstream.released.store(true, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if state.jobs.list().await.unwrap().iter().all(|j| {
                matches!(
                    j.status,
                    verruckt_lib::domain::JobStatus::Completed
                        | verruckt_lib::domain::JobStatus::Failed
                )
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(courses::get_course(&state.pool, deleted.id).await.is_err());
    assert!(courses::list_nodes(&state.pool, deleted.id)
        .await
        .unwrap()
        .is_empty());
    server.abort();
}
