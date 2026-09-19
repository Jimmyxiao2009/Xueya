//! Desktop integration coverage for the experimental AppData-backed vector backend.

use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;
use verruckt_lib::db::repo::{chunks as chunk_repo, sources as source_repo};
use verruckt_lib::domain::SourceKind;
use verruckt_lib::knowledge::embedding_cache;
use verruckt_lib::knowledge::hybrid::{HybridRetriever, RetrievalFilter};
use verruckt_lib::secrets::keyring_store::InMemorySecretStore;
use verruckt_lib::services::course_service::{CourseService, RouteRequest};
use verruckt_lib::services::ingestion_service::{IngestInput, IngestionService};
use verruckt_lib::services::source_service;
use verruckt_lib::services::tutor_service::TutorService;
use verruckt_lib::state::{AppPaths, AppState, VectorBackend};

async fn sqlite_state(dir: &TempDir) -> AppState {
    AppState::new_with_secrets_and_backend(
        AppPaths::new(dir.path()),
        Arc::new(InMemorySecretStore::new()),
        VectorBackend::Sqlite,
    )
    .await
    .expect("sqlite app state")
}

async fn create_route(state: &AppState, topic: &str, source_id: Uuid) -> Uuid {
    CourseService::new(state)
        .create_course(
            RouteRequest {
                topic: topic.to_owned(),
                source_ids: vec![source_id],
                ..RouteRequest::new(topic)
            },
            None,
        )
        .await
        .expect("route")
        .id
}

async fn state(dir: &TempDir) -> AppState {
    AppState::new_with_secrets_and_backend(
        AppPaths::new(dir.path()),
        Arc::new(InMemorySecretStore::new()),
        VectorBackend::Verruckt,
    )
    .await
    .expect("verruckt app state")
}

async fn ingest(state: &AppState, text: &str) -> Uuid {
    IngestionService::new(state)
        .ingest_bytes(
            IngestInput {
                filename: "projective.md".into(),
                bytes: text.as_bytes().to_vec(),
                kind: SourceKind::File,
                title: Some("Projective Geometry".into()),
                uri: None,
                mime_type: Some("text/markdown".into()),
            },
            None,
        )
        .await
        .expect("ingestion")
        .id
}

#[tokio::test]
async fn verruckt_ingestion_marks_ready_and_uses_appdata_space() {
    let dir = tempfile::tempdir().unwrap();
    let state = state(&dir).await;
    let source_id = ingest(
        &state,
        "# Projective Geometry\nParallel lines meet at an ideal point at infinity in the projective plane.",
    )
    .await;

    let source = source_repo::get_source(&state.pool, source_id)
        .await
        .unwrap();
    assert_eq!(source.status, verruckt_lib::domain::SourceStatus::Ready);
    let ready: i64 = embedding_cache::count_source_index_status(&state.pool, source_id, "ready")
        .await
        .unwrap();
    assert_eq!(ready, 1);
    assert!(dir.path().join("vector").is_dir());
    assert!(std::fs::read_dir(dir.path().join("vector"))
        .unwrap()
        .next()
        .is_some());

    let chunks = sqlx::query_scalar::<_, Uuid>(
        "SELECT c.id FROM chunks c JOIN documents d ON d.id = c.document_id WHERE d.source_id = ?",
    )
    .bind(source_id)
    .fetch_all(&state.pool)
    .await
    .unwrap();
    assert!(!chunks.is_empty());
}

#[tokio::test]
async fn verruckt_hybrid_route_search_survives_appstate_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let app_state = state(&dir).await;
    let source_id = ingest(
        &app_state,
        "# Projective Geometry\nParallel lines in the projective plane meet at an ideal point at infinity.",
    )
    .await;
    let route_id = CourseService::new(&app_state)
        .create_course(
            RouteRequest {
                topic: "Projective Geometry".into(),
                source_ids: vec![source_id],
                ..RouteRequest::new("Projective Geometry")
            },
            None,
        )
        .await
        .unwrap()
        .id;

    let hits = HybridRetriever::new(&app_state)
        .retrieve(
            "Where do parallel lines meet in the projective plane?",
            RetrievalFilter::for_route(route_id).with_top_k(5),
        )
        .await
        .unwrap();
    assert!(!hits.is_empty());
    assert!(hits.iter().all(|hit| hit.source_id == source_id));

    drop(app_state);
    let reopened = state(&dir).await;
    let reopened_hits = HybridRetriever::new(&reopened)
        .retrieve(
            "Where do parallel lines meet in the projective plane?",
            RetrievalFilter::for_route(route_id).with_top_k(5),
        )
        .await
        .unwrap();
    assert!(!reopened_hits.is_empty());
    assert!(reopened_hits.iter().all(|hit| hit.source_id == source_id));
}

#[tokio::test]
async fn deleting_source_removes_authoritative_and_derived_vectors() {
    let dir = tempfile::tempdir().unwrap();
    let state = state(&dir).await;
    let source_id = ingest(&state, "A source that will be deleted.").await;
    let chunk_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT c.id FROM chunks c JOIN documents d ON d.id = c.document_id WHERE d.source_id = ?",
    )
    .bind(source_id)
    .fetch_all(&state.pool)
    .await
    .unwrap();
    assert!(!chunk_ids.is_empty());

    for chunk_id in &chunk_ids {
        state.vector_index.delete(*chunk_id).await.unwrap();
    }
    source_service::delete_source(&state, source_id)
        .await
        .unwrap();
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chunks WHERE id = ?")
        .bind(chunk_ids[0])
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0);

    let _ = chunk_repo::get_chunk(&state.pool, chunk_ids[0]).await.err();
}

#[tokio::test]
async fn default_constructor_keeps_sqlite_backend() {
    let dir = tempfile::tempdir().unwrap();
    let state = sqlite_state(&dir).await;
    let source_id = ingest(&state, "SQLite remains the production default.").await;
    let ready: i64 = embedding_cache::count_source_index_status(&state.pool, source_id, "ready")
        .await
        .unwrap();
    assert_eq!(ready, 1);
    // The SQLite backend never writes the experimental AppData vector tree.
    assert!(!dir.path().join("vector").exists());
}

#[tokio::test]
async fn verruckt_top1_and_citation_grounding_for_projective_query() {
    let dir = tempfile::tempdir().unwrap();
    let state = state(&dir).await;
    let source_id = ingest(
        &state,
        "# Projective Geometry
平行线在射影平面中最终相交于无穷远点。齐次坐标 [1:2:0] 表示方向 (1,2) 的无穷远点。",
    )
    .await;
    let route_id = create_route(&state, "射影几何", source_id).await;

    let hits = HybridRetriever::new(&state)
        .retrieve(
            "为什么平行线在射影平面中会相交？",
            RetrievalFilter::for_route(route_id).with_top_k(5),
        )
        .await
        .unwrap();
    assert!(!hits.is_empty(), "expected a grounded hit");
    let top = &hits[0];
    assert_eq!(top.source_id, source_id);
    assert!(
        hits.iter().any(|hit| hit.text.contains("无穷远点")),
        "expected the infinity chunk in the top results: {hits:?}"
    );

    // Tutor citations resolve back to the retrieved chunk.
    let nodes = verruckt_lib::db::repo::courses::list_nodes(&state.pool, route_id)
        .await
        .unwrap();
    let started = verruckt_lib::services::learning_service::LearningService::new(&state)
        .start(nodes[0].id, None)
        .await
        .unwrap();
    let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();
    let turn = TutorService::new(&state)
        .turn_stream(
            started.session.id,
            "根据我的资料，为什么平行线在射影平面中会相交？",
            sender,
        )
        .await
        .unwrap();
    assert_eq!(turn.grounding, "source_grounded");
    assert!(!turn.citations.is_empty());
    assert!(
        turn.citations
            .iter()
            .any(|citation| citation.text.contains("无穷远点")),
        "citation should point at the retrieved chunk: {:?}",
        turn.citations
    );
}

#[tokio::test]
async fn reindex_reuses_embedding_cache_without_new_provider_calls() {
    let dir = tempfile::tempdir().unwrap();
    let app_state = state(&dir).await;
    let source_id = ingest(
        &app_state,
        "缓存复用测试：重新索引同一 source 不应该产生新的 embedding 调用。",
    )
    .await;
    let embeddings_before = embedding_cache::count_embeddings(&app_state.pool)
        .await
        .unwrap();
    let cached_before = embedding_cache::count_cached_embeddings(&app_state.pool)
        .await
        .unwrap();
    assert!(cached_before > 0);

    IngestionService::new(&app_state)
        .reindex_source(source_id, None)
        .await
        .unwrap();

    let embeddings_after = embedding_cache::count_embeddings(&app_state.pool)
        .await
        .unwrap();
    let cached_after = embedding_cache::count_cached_embeddings(&app_state.pool)
        .await
        .unwrap();
    assert_eq!(embeddings_before, embeddings_after);
    assert_eq!(cached_before, cached_after);
    let ready: i64 =
        embedding_cache::count_source_index_status(&app_state.pool, source_id, "ready")
            .await
            .unwrap();
    assert_eq!(ready, 1);
}

#[tokio::test]
async fn corrupt_derived_index_recovers_from_sqlite_on_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let app_state = state(&dir).await;
    let source_id = ingest(
        &app_state,
        "# Projective Geometry
Parallel lines meet at an ideal point at infinity.",
    )
    .await;
    let vectors_dir = dir.path().join("vector");
    let space_dir = std::fs::read_dir(&vectors_dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    drop(app_state);
    // Corrupt the manifest of the derived index; SQLite embeddings are intact.
    let manifest = space_dir.join("manifest.bin");
    let mut bytes = std::fs::read(&manifest).unwrap();
    bytes[0] ^= 0xFF;
    std::fs::write(&manifest, &bytes).unwrap();

    let reopened = state(&dir).await;
    let route_id = create_route(&reopened, "射影几何", source_id).await;
    let hits = HybridRetriever::new(&reopened)
        .retrieve(
            "Where do parallel lines meet?",
            RetrievalFilter::for_route(route_id).with_top_k(5),
        )
        .await
        .unwrap();
    assert!(
        !hits.is_empty(),
        "derived index must be rebuilt from SQLite"
    );
    let ready: i64 = embedding_cache::count_source_index_status(&reopened.pool, source_id, "ready")
        .await
        .unwrap();
    assert_eq!(ready, 1);
}

#[tokio::test]
async fn interrupted_index_is_not_ready_until_reindexed() {
    let dir = tempfile::tempdir().unwrap();
    let app_state = state(&dir).await;
    let source_id = ingest(&app_state, "中断的索引状态测试。").await;
    let (provider_id, model, dimension): (Uuid, String, i64) = sqlx::query_as(
        "SELECT provider_id, model, dimension FROM source_index_state WHERE source_id = ?",
    )
    .bind(source_id)
    .fetch_one(&app_state.pool)
    .await
    .unwrap();
    embedding_cache::mark_source_index_status(
        &app_state.pool,
        source_id,
        provider_id,
        &model,
        dimension as usize,
        "interrupted",
    )
    .await
    .unwrap();
    let ready: i64 =
        embedding_cache::count_source_index_status(&app_state.pool, source_id, "ready")
            .await
            .unwrap();
    assert_eq!(ready, 0, "interrupted index must not report ready");

    IngestionService::new(&app_state)
        .reindex_source(source_id, None)
        .await
        .unwrap();
    let ready_after: i64 =
        embedding_cache::count_source_index_status(&app_state.pool, source_id, "ready")
            .await
            .unwrap();
    assert_eq!(ready_after, 1);
}
