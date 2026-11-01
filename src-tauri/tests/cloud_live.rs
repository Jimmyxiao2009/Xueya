//! Real end-to-end test against a running Verruckt Cloud and a configured
//! upstream model. This test must never silently accept the local mock.
//!
//! Run with:
//!   $env:VERRUCKT_CLOUD_URL = "http://127.0.0.1:1431"
//!   $env:VERRUCKT_E2E_EMAIL = "demo@example.com"
//!   $env:VERRUCKT_E2E_PASSWORD = "..."
//!   cargo test --test cloud_live -- --ignored --nocapture
//!
//! Set `VERRUCKT_E2E_REQUIRE_EMBEDDINGS=1` when the configured upstream also
//! exposes embeddings. Chat and streaming are always required.

use futures_util::StreamExt;
use std::sync::Arc;
use std::time::Instant;
use tempfile::TempDir;
use uuid::Uuid;
use verruckt_lib::ai::stream::{with_lifecycle, AiStreamEvent};
use verruckt_lib::ai::traits::{ChatMessage, ChatRequest};
use verruckt_lib::domain::SourceKind;
use verruckt_lib::knowledge_fs::blob_store::BlobStore;
use verruckt_lib::secrets::keyring_store::{InMemorySecretStore, SecretStore};
use verruckt_lib::db::repo::{courses as course_repo, learning_loop as loop_repo};
use verruckt_lib::services::course_service::{CourseService, RouteRequest};
use verruckt_lib::services::ingestion_service::{IngestInput, IngestionService};
use verruckt_lib::services::learning_service::LearningService;
use verruckt_lib::services::tutor_service::TutorService;
use verruckt_lib::state::{AppPaths, AppState};

async fn make_state(secrets: Arc<dyn SecretStore>, dir: &TempDir) -> AppState {
    AppState::new_with_secrets(AppPaths::new(dir.path()), secrets)
        .await
        .expect("app state")
}

#[tokio::test]
#[ignore = "requires docker compose up and VERRUCKT_E2E_EMAIL"]
async fn live_cloud_account_ai_sync_files_and_persistence() {
    let base_url =
        std::env::var("VERRUCKT_CLOUD_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_owned());
    let Ok(email) = std::env::var("VERRUCKT_E2E_EMAIL") else {
        eprintln!("VERRUCKT_E2E_EMAIL is not set; skipping live cloud test");
        return;
    };
    let password =
        std::env::var("VERRUCKT_E2E_PASSWORD").unwrap_or_else(|_| "password123".to_owned());

    // ---- Account + entitlements ------------------------------------------
    let dir_a = tempfile::tempdir().expect("tempdir a");
    let secrets_a: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let state_a = make_state(secrets_a.clone(), &dir_a).await;
    state_a
        .cloud
        .set_base_url(&base_url)
        .await
        .expect("set base url");

    let account = state_a
        .cloud
        .login(email.clone(), password.clone())
        .await
        .expect("cloud login");
    assert!(account.logged_in, "login should produce a session");
    let entitlements = state_a.cloud.entitlements().await.expect("entitlements");
    assert!(
        entitlements.feature("official_ai"),
        "pro account needs official_ai"
    );
    assert!(
        entitlements.feature("cloud_sync"),
        "pro account needs cloud_sync"
    );
    state_a
        .cloud
        .set_sync_enabled(true)
        .await
        .expect("enable sync");

    // ---- Cloud AI provider ----------------------------------------------
    sqlx::query(
        "INSERT INTO app_settings (key, value_json, updated_at) VALUES ('ai_provider_mode', ?, ?) \
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
    )
    .bind(serde_json::to_string("cloud").expect("json"))
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&state_a.pool)
    .await
    .expect("set cloud mode");

    let (config, chat) = state_a
        .providers
        .default_chat_provider()
        .await
        .expect("cloud chat provider");
    assert_eq!(config.id, verruckt_lib::ai::registry::CLOUD_PROVIDER_ID);
    assert_eq!(config.display_name, "学涯云");
    assert!(config.base_url.starts_with(&base_url));
    let response = chat
        .chat(ChatRequest::new(
            config.chat_model.clone(),
            vec![ChatMessage::user("hello verruckt e2e")],
        ))
        .await
        .expect("cloud chat");
    assert!(
        !response.content.trim().is_empty(),
        "real upstream returned an empty cloud chat response"
    );
    assert_ne!(
        response.content.trim(),
        "mock upstream reply",
        "the competition E2E must not silently use the local mock upstream"
    );

    // Desktop SSE parser -> Cloud relay -> configured real upstream -> Cloud
    // relay -> Desktop. The lifecycle must be Started, Delta*, Completed with
    // no duplicate deltas or a second terminal event.
    let stream = chat
        .chat_stream(ChatRequest::new(
            config.chat_model.clone(),
            vec![ChatMessage::user("stream verruckt e2e")],
        ))
        .await
        .expect("cloud stream");
    let mut events = with_lifecycle(stream, config.chat_model.clone());
    let mut collected = Vec::new();
    while let Some(event) = events.next().await {
        collected.push(event.expect("stream event"));
    }
    assert!(matches!(
        collected.first(),
        Some(AiStreamEvent::Started { .. })
    ));
    assert!(matches!(
        collected.last(),
        Some(AiStreamEvent::Completed { .. })
    ));
    let delta_text: String = collected
        .iter()
        .filter_map(|event| match event {
            AiStreamEvent::Delta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        !delta_text.trim().is_empty(),
        "real upstream returned no visible streaming content: {collected:?}"
    );
    assert_ne!(
        delta_text.trim(),
        "mock upstream reply",
        "the competition streaming E2E must not use the local mock upstream"
    );
    assert_eq!(
        collected
            .iter()
            .filter(|event| matches!(event, AiStreamEvent::Completed { .. }))
            .count(),
        1,
        "stream must have exactly one terminal event: {collected:?}"
    );

    let embedding_result = match state_a.providers.default_embedding_provider().await {
        Ok((_, embedding)) => embedding
            .embed(vec!["hello verruckt e2e".to_owned()])
            .await,
        Err(error) => Err(error),
    };
    match embedding_result {
        Ok(vectors) => assert!(
            vectors.first().map(|vector| !vector.is_empty()).unwrap_or(false),
            "real upstream returned an empty embedding"
        ),
        Err(error) if std::env::var("VERRUCKT_E2E_REQUIRE_EMBEDDINGS").as_deref() != Ok("1") => {
            eprintln!("SKIP embeddings: {}", error.public_message());
        }
        Err(error) => panic!("cloud embedding required but failed: {error:?}"),
    }

    // ---- Delta sync A -> cloud -> B -------------------------------------
    let topic = format!("E2E Sync Topic {}", Uuid::new_v4());
    let course_a = CourseService::new(&state_a)
        .create_course(RouteRequest::new(topic.clone()), None)
        .await
        .expect("create course");
    let push_a = state_a.sync.sync_now().await.expect("first sync");
    assert!(
        push_a.pushed > 0,
        "expected local changes to be pushed: {push_a:?}"
    );

    let dir_b = tempfile::tempdir().expect("tempdir b");
    let secrets_b: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let state_b = make_state(secrets_b, &dir_b).await;
    state_b
        .cloud
        .set_base_url(&base_url)
        .await
        .expect("set base url b");
    state_b
        .cloud
        .login(email.clone(), password.clone())
        .await
        .expect("cloud login b");
    state_b
        .cloud
        .set_sync_enabled(true)
        .await
        .expect("enable sync b");
    let pull_b = state_b.sync.sync_now().await.expect("second sync");
    assert!(!pull_b.skipped, "device B sync should run: {pull_b:?}");

    let courses_b = CourseService::new(&state_b)
        .list_courses()
        .await
        .expect("list courses b");
    let course_b = courses_b
        .iter()
        .find(|course| course.topic == topic)
        .expect("synced course should exist on device B");

    // ---- Conflict: never silently overwrite the local copy --------------
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("UPDATE courses SET title = 'A server title', updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(course_a.id)
        .execute(&state_a.pool)
        .await
        .expect("server edit");
    state_a.sync.sync_now().await.expect("push server edit");

    sqlx::query("UPDATE courses SET title = 'B local title', updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(course_b.id)
        .execute(&state_b.pool)
        .await
        .expect("local edit");
    state_b.sync.sync_now().await.expect("conflicting sync");
    let conflicts = state_b.sync.conflicts().await.expect("conflicts");
    assert!(!conflicts.is_empty(), "expected a recorded sync conflict");
    let local_title: String = sqlx::query_scalar("SELECT title FROM courses WHERE id = ?")
        .bind(course_b.id)
        .fetch_one(&state_b.pool)
        .await
        .expect("local title");
    assert_eq!(
        local_title, "B local title",
        "local data must survive a sync conflict"
    );

    // ---- Source file sync through object storage -------------------------
    let source = IngestionService::new(&state_a)
        .ingest_bytes(
            IngestInput {
                filename: "e2e-notes.txt".to_owned(),
                bytes: b"E2E source content about vectors and geometry".to_vec(),
                kind: SourceKind::File,
                title: Some("E2E Notes".to_owned()),
                uri: None,
                mime_type: Some("text/plain".to_owned()),
            },
            None,
        )
        .await
        .expect("ingest source");
    state_a.sync.sync_now().await.expect("push source metadata");
    let upload = state_a.files.sync().await.expect("upload file");
    if std::env::var("VERRUKT_E2E_REQUIRE_STORAGE").as_deref() == Ok("1") {
        assert_eq!(upload.uploaded, 1, "expected one uploaded file: {upload:?}");
        state_a.sync.sync_now().await.expect("push cloud file id");

        state_b.sync.sync_now().await.expect("pull source metadata");
        let download = state_b.files.sync().await.expect("download file");
        assert_eq!(download.downloaded, 1, "expected one downloaded file: {download:?}");
        let hash = source.content_hash.expect("source hash");
        let relative_path = BlobStore::relative_path_for_hash(&hash);
        assert!(state_b.blobs.exists(&relative_path).await, "device B should have the verified blob");
        IngestionService::new(&state_b)
            .reindex_source(source.id, None)
            .await
            .expect("rebuild derived chunks and embeddings on device B");
    } else {
        eprintln!("SKIP file object-storage E2E: set VERRUKT_E2E_REQUIRE_STORAGE=1 to require it");
    }

    // ---- Session persistence across a restart ---------------------------
    state_a.pool.close().await;
    drop(state_a);
    let reopened = make_state(secrets_a, &dir_a).await;
    let restored = reopened
        .cloud
        .account_state()
        .await
        .expect("restore account state");
    assert!(restored.logged_in, "refresh token should survive a restart");
    assert!(reopened
        .cloud
        .cached_entitlements()
        .await
        .expect("cached entitlements")
        .feature("cloud_sync"));
}

/// Competition-path E2E: a real Cloud model authors the course, opens a Local
/// LearningSession, and handles ten live teaching turns. The conversation is
/// intentionally kept in SQLite; Cloud only supplies the model response.
#[tokio::test]
#[ignore = "requires a real Cloud upstream and a Pro demo account"]
async fn live_cloud_teaching_session_ten_turns() {
    let base_url = std::env::var("VERRUCKT_CLOUD_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8080".to_owned());
    let Ok(email) = std::env::var("VERRUCKT_E2E_EMAIL") else {
        eprintln!("VERRUCKT_E2E_EMAIL is not set; skipping live teaching test");
        return;
    };
    let password = std::env::var("VERRUCKT_E2E_PASSWORD")
        .unwrap_or_else(|_| "password123".to_owned());

    let dir = tempfile::tempdir().expect("demo tempdir");
    let secrets: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let state = make_state(secrets.clone(), &dir).await;
    state
        .cloud
        .set_base_url(&base_url)
        .await
        .expect("set base url");
    let account = state
        .cloud
        .login(email, password)
        .await
        .expect("cloud login");
    assert!(account.logged_in, "demo account must be authenticated");
    assert!(
        state
            .cloud
            .cached_entitlements()
            .await
            .expect("cached entitlements")
            .feature("official_ai"),
        "demo account must have official_ai entitlement"
    );
    sqlx::query(
        "INSERT INTO app_settings (key, value_json, updated_at) VALUES ('ai_provider_mode', ?, ?) \
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
    )
    .bind(serde_json::to_string("cloud").expect("json"))
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&state.pool)
    .await
    .expect("select Cloud provider");

    let course_started = Instant::now();
    let course = CourseService::new(&state)
        .create_course(
            RouteRequest::new("光的折射：为什么筷子在水中看起来是弯的"),
            None,
        )
        .await
        .expect("real Cloud course generation");
    let nodes = course_repo::list_nodes(&state.pool, course.id)
        .await
        .expect("course nodes");
    let node = nodes.first().expect("course needs a first node").clone();
    eprintln!(
        "DEMO_COURSE course_id={} nodes={} latency_ms={}",
        course.id,
        nodes.len(),
        course_started.elapsed().as_millis()
    );

    let session_started = Instant::now();
    let started = LearningService::new(&state)
        .start(node.id, None)
        .await
        .expect("open Local LearningSession");
    let opening = TutorService::new(&state)
        .opening(started.session.id)
        .await
        .expect("real Cloud Agent opening");
    assert!(!opening.message.trim().is_empty(), "opening must not be empty");
    assert_ne!(opening.generated_by, "mock", "opening must come from real Cloud");
    eprintln!(
        "DEMO_OPENING session_id={} latency_ms={} action={} text={}",
        started.session.id,
        session_started.elapsed().as_millis(),
        opening.suggested_action,
        opening.message.replace('\n', " ").chars().take(100).collect::<String>()
    );

    let scripted_turns = [
        "我觉得筷子是真的被水折断了。",
        "不知道。",
        "直接给我讲。",
        "如果光线从水进入空气，会发生什么？",
        "我认为光线会一直沿原来的直线走。",
        "是不是因为水和空气的折射率不同？",
        "还是不明白。",
        "请用一个生活中的例子解释。",
        "所以我们看到的位置不是物体的实际位置，对吗？",
        "我可以用折射解释筷子看起来弯曲。",
    ];
    let mut actions = Vec::with_capacity(scripted_turns.len());
    for (index, message) in scripted_turns.iter().enumerate() {
        let started_at = Instant::now();
        let turn = TutorService::new(&state)
            .turn(started.session.id, message)
            .await
            .unwrap_or_else(|error| panic!("Cloud teaching turn {} failed: {error:?}", index + 1));
        assert!(!turn.message.trim().is_empty(), "turn {} was empty", index + 1);
        assert_ne!(turn.generated_by, "mock", "turn {} used the mock provider", index + 1);
        assert!(!turn.suggested_action.trim().is_empty(), "turn {} had no action", index + 1);
        actions.push(turn.suggested_action.clone());
        eprintln!(
            "DEMO_TURN index={} latency_ms={} action={} text={}",
            index + 1,
            started_at.elapsed().as_millis(),
            turn.suggested_action,
            turn.message.replace('\n', " ").chars().take(120).collect::<String>()
        );
    }
    assert_eq!(actions.len(), 10);
    let records = loop_repo::list_messages(&state.pool, started.session.id)
        .await
        .expect("conversation history");
    assert_eq!(records.len(), 21, "opening + ten user/agent turns must persist");

    state
        .pool
        .close()
        .await;
    drop(state);
    let reopened = make_state(secrets, &dir).await;
    let restored = reopened
        .cloud
        .account_state()
        .await
        .expect("restore Cloud session");
    assert!(restored.logged_in, "refresh token must restore after restart");
    let turns = TutorService::new(&reopened)
        .turns(started.session.id)
        .await
        .expect("restored conversation turns");
    assert_eq!(turns.len(), 11, "opening plus ten agent turns must resume");
    LearningService::new(&reopened)
        .finish(started.session.id)
        .await
        .expect("finish demo session");
    eprintln!("DEMO_TEACHING PASS turns=10 persisted=true resumed=true");
}
