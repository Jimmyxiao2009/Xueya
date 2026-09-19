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
use verruckt_lib::state::{AppPaths, AppState, VectorBackend};

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
