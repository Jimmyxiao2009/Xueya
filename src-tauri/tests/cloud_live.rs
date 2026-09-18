//! Real end-to-end test against a running Verruckt Cloud and the Docker
//! Compose mock OpenAI upstream.
//!
//! Run with:
//!   $env:VERRUCKT_CLOUD_URL = "http://127.0.0.1:8080"
//!   $env:VERRUCKT_E2E_EMAIL = "e2e@example.com"
//!   $env:VERRUCKT_E2E_PASSWORD = "password123"
//!   cargo test --test cloud_live -- --ignored --nocapture
//!
//! The harness script upgrades the account to `pro` before running this test;
//! the test itself never touches PostgreSQL directly.

use futures_util::StreamExt;
use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;
use verruckt_lib::ai::stream::{with_lifecycle, AiStreamEvent};
use verruckt_lib::ai::traits::{ChatMessage, ChatRequest};
use verruckt_lib::domain::SourceKind;
use verruckt_lib::knowledge_fs::blob_store::BlobStore;
use verruckt_lib::secrets::keyring_store::{InMemorySecretStore, SecretStore};
use verruckt_lib::services::course_service::{CourseService, RouteRequest};
use verruckt_lib::services::ingestion_service::{IngestInput, IngestionService};
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
    assert_eq!(config.display_name, "Verruckt Cloud");
    let response = chat
        .chat(ChatRequest::new(
            config.chat_model.clone(),
            vec![ChatMessage::user("hello verruckt e2e")],
        ))
        .await
        .expect("cloud chat");
    assert!(
        response.content.contains("mock upstream reply"),
        "unexpected cloud chat response: {}",
        response.content
    );

    // Desktop SSE parser -> Cloud relay -> mock upstream -> Cloud relay ->
    // Desktop. The lifecycle must be Started, Delta*, Completed with no
    // duplicate deltas or a second terminal event.
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
        delta_text.contains("mock upstream reply"),
        "unexpected cloud stream deltas: {collected:?}"
    );
    assert_eq!(
        collected
            .iter()
            .filter(|event| matches!(event, AiStreamEvent::Completed { .. }))
            .count(),
        1,
        "stream must have exactly one terminal event: {collected:?}"
    );

    let (_, embedding) = state_a
        .providers
        .default_embedding_provider()
        .await
        .expect("cloud embedding provider");
    let vectors = embedding
        .embed(vec!["hello verruckt e2e".to_owned()])
        .await
        .expect("cloud embedding");
    assert_eq!(vectors[0].len(), 8, "mock upstream returns 8 dimensions");

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
    assert_eq!(upload.uploaded, 1, "expected one uploaded file: {upload:?}");
    state_a.sync.sync_now().await.expect("push cloud file id");

    state_b.sync.sync_now().await.expect("pull source metadata");
    let download = state_b.files.sync().await.expect("download file");
    assert_eq!(
        download.downloaded, 1,
        "expected one downloaded file: {download:?}"
    );
    let hash = source.content_hash.expect("source hash");
    let relative_path = BlobStore::relative_path_for_hash(&hash);
    assert!(
        state_b.blobs.exists(&relative_path).await,
        "device B should have the verified blob"
    );
    IngestionService::new(&state_b)
        .reindex_source(source.id, None)
        .await
        .expect("rebuild derived chunks and embeddings on device B");

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
