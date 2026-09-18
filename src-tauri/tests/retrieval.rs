//! Retrieval and indexing integration tests.
//!
//! These deliberately exercise the local-only path: a mock provider is
//! installed by `AppState::new_with_secrets`, no network is required, and the
//! assertions are about which local chunks survive route scoping.

use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;
use verruckt_lib::ai::stream::AiStreamEvent;
use verruckt_lib::db::repo::{
    chunks as chunk_repo, courses as course_repo, sources as source_repo,
};
use verruckt_lib::domain::SourceKind;
use verruckt_lib::knowledge::embedding_cache;
use verruckt_lib::knowledge::hybrid::{HybridRetriever, RetrievalFilter};
use verruckt_lib::secrets::keyring_store::InMemorySecretStore;
use verruckt_lib::services::course_service::{CourseService, RouteRequest};
use verruckt_lib::services::ingestion_service::{IngestInput, IngestionService};
use verruckt_lib::services::learning_service::LearningService;
use verruckt_lib::services::tutor_service::TutorService;
use verruckt_lib::state::{AppPaths, AppState};

async fn test_state() -> (AppState, TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let state = AppState::new_with_secrets(
        AppPaths::new(dir.path()),
        Arc::new(InMemorySecretStore::new()),
    )
    .await
    .expect("app state");
    (state, dir)
}

async fn ingest_text(state: &AppState, filename: &str, text: &str) -> Uuid {
    let source = IngestionService::new(state)
        .ingest_bytes(
            IngestInput {
                filename: filename.to_owned(),
                bytes: text.as_bytes().to_vec(),
                kind: SourceKind::File,
                title: None,
                uri: None,
                mime_type: Some("text/plain".to_owned()),
            },
            None,
        )
        .await
        .expect("ingest");
    let documents = source_repo::list_documents_by_source(&state.pool, source.id)
        .await
        .expect("documents");
    let mut chunk_ids = Vec::new();
    for document in documents {
        let chunks = chunk_repo::list_chunks_by_document(&state.pool, document.id)
            .await
            .expect("chunks");
        chunk_ids.extend(chunks.into_iter().map(|chunk| chunk.id));
    }
    IngestionService::new(state)
        .embed_chunks(&chunk_ids, None)
        .await
        .expect("embed");
    source.id
}

async fn create_route(state: &AppState, topic: &str, source_ids: Vec<Uuid>) -> Uuid {
    CourseService::new(state)
        .create_course(
            RouteRequest {
                topic: topic.to_owned(),
                source_ids,
                ..RouteRequest::new(topic)
            },
            None,
        )
        .await
        .expect("route")
        .id
}

#[tokio::test]
async fn hybrid_retrieval_finds_projective_infinity_and_keeps_routes_isolated() {
    let (state, _dir) = test_state().await;
    let geometry_source = ingest_text(
        &state,
        "projective.txt",
        "平行线在射影平面中最终相交于无穷远点。齐次坐标 [1:2:0] 表示方向 (1,2) 的无穷远点。",
    )
    .await;
    let algebra_source = ingest_text(
        &state,
        "linear.txt",
        "线性代数研究向量空间和矩阵，线性方程组可能有唯一解、无解或无穷多解。",
    )
    .await;

    let geometry_route = create_route(&state, "射影几何", vec![geometry_source]).await;
    let algebra_route = create_route(&state, "线性代数", vec![algebra_source]).await;

    let retriever = HybridRetriever::new(&state);
    let geometry_hits = retriever
        .retrieve(
            "平行线最终在哪里相交",
            RetrievalFilter::for_route(geometry_route).with_top_k(6),
        )
        .await
        .expect("geometry retrieval");
    assert!(!geometry_hits.is_empty(), "expected a geometry hit");
    assert!(geometry_hits
        .iter()
        .any(|hit| hit.text.contains("无穷远点")));
    assert!(geometry_hits
        .iter()
        .all(|hit| hit.source_id == geometry_source));

    let coordinate_hits = retriever
        .retrieve(
            "[1:2:0]",
            RetrievalFilter::for_route(geometry_route).with_top_k(6),
        )
        .await
        .expect("coordinate retrieval");
    assert!(!coordinate_hits.is_empty(), "expected lexical [1:2:0] hit");
    assert!(coordinate_hits.iter().any(|hit| hit.lexical_rank.is_some()));

    // Route isolation: a query about vectors must not leak algebra material
    // into the geometry route even though the vector index holds both.
    let leaked = retriever
        .retrieve(
            "向量空间和矩阵",
            RetrievalFilter::for_route(geometry_route).with_top_k(6),
        )
        .await
        .expect("isolated retrieval");
    assert!(
        leaked.iter().all(|hit| hit.source_id != algebra_source),
        "route A leaked route B material: {leaked:?}"
    );

    let algebra_hits = retriever
        .retrieve(
            "向量空间和矩阵",
            RetrievalFilter::for_route(algebra_route).with_top_k(6),
        )
        .await
        .expect("algebra retrieval");
    assert!(!algebra_hits.is_empty());
    assert!(algebra_hits
        .iter()
        .all(|hit| hit.source_id == algebra_source));
}

#[tokio::test]
async fn re_embedding_identical_chunks_uses_the_cache() {
    let (state, _dir) = test_state().await;
    let source_id = ingest_text(
        &state,
        "cache.txt",
        "同一段文字第二次索引时不应该再次调用嵌入服务，缓存键包含 provider 身份与模型。",
    )
    .await;
    let documents = source_repo::list_documents_by_source(&state.pool, source_id)
        .await
        .expect("documents");
    let mut chunk_ids = Vec::new();
    for document in documents {
        chunk_ids.extend(
            chunk_repo::list_chunks_by_document(&state.pool, document.id)
                .await
                .expect("chunks")
                .into_iter()
                .map(|chunk| chunk.id),
        );
    }
    let first_count = embedding_cache::count_embeddings(&state.pool)
        .await
        .expect("count");
    let cache_count = embedding_cache::count_cached_embeddings(&state.pool)
        .await
        .expect("cache count");
    assert!(cache_count > 0);

    IngestionService::new(&state)
        .embed_chunks(&chunk_ids, None)
        .await
        .expect("re-embed");
    let second_count = embedding_cache::count_embeddings(&state.pool)
        .await
        .expect("count");
    assert_eq!(first_count, second_count);

    let metadata_count = embedding_cache::count_index_metadata(&state.pool)
        .await
        .expect("metadata");
    assert!(metadata_count > 0);
    let ready_count = embedding_cache::count_source_index_status(&state.pool, source_id, "ready")
        .await
        .expect("source state");
    assert!(ready_count > 0);

    // A provider/model dimension change must invalidate the old source index
    // instead of mixing old and new vectors in one search.
    let (config, _embedding) = state
        .providers
        .default_embedding_provider()
        .await
        .expect("embedding provider");
    let metadata =
        embedding_cache::get_index_metadata(&state.pool, config.id, &config.embedding_model)
            .await
            .expect("index metadata")
            .expect("metadata row");
    embedding_cache::upsert_index_metadata(
        &state.pool,
        metadata.provider_id,
        &metadata.model,
        metadata.dimension + 1,
    )
    .await
    .expect("bump dimension");
    IngestionService::new(&state)
        .embed_chunks(&chunk_ids, None)
        .await
        .expect("re-embed after dimension change");
    let ready_after = embedding_cache::count_source_index_status(&state.pool, source_id, "ready")
        .await
        .expect("ready after");
    assert!(ready_after > 0);
    assert_eq!(
        embedding_cache::count_embeddings(&state.pool)
            .await
            .expect("embeddings after"),
        second_count
    );
}

#[test]
fn route_request_preserves_source_ids() {
    let request = RouteRequest {
        topic: "射影几何".to_owned(),
        source_ids: vec![Uuid::new_v4()],
        ..RouteRequest::new("射影几何")
    };
    assert_eq!(request.source_ids.len(), 1);
}

#[tokio::test]
async fn streaming_tutor_citations_map_back_to_the_retrieved_chunk() {
    let (state, _dir) = test_state().await;
    let source_id = ingest_text(
        &state,
        "projective.txt",
        "平行线在射影平面中最终相交于无穷远点。齐次坐标 [1:2:0] 表示方向 (1,2) 的无穷远点。",
    )
    .await;
    let route_id = create_route(&state, "射影几何", vec![source_id]).await;
    let nodes = course_repo::list_nodes(&state.pool, route_id)
        .await
        .expect("nodes");
    let started = LearningService::new(&state)
        .start(nodes[0].id, None)
        .await
        .expect("session");

    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let turn = TutorService::new(&state)
        .turn_stream(
            started.session.id,
            "根据我的资料，平行线最终在哪里相交？",
            sender,
        )
        .await
        .expect("streamed tutor turn");

    assert!(!turn.message.trim().is_empty());
    assert_eq!(turn.grounding, "source_grounded");
    assert!(
        !turn.citations.is_empty(),
        "expected the streamed answer to cite the local source"
    );
    assert!(
        turn.citations
            .iter()
            .any(|citation| citation.text.contains("无穷远点")),
        "citation should point at the retrieved chunk: {:?}",
        turn.citations
    );

    let mut events = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        events.push(event);
    }
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AiStreamEvent::Delta { text } if !text.is_empty())),
        "expected at least one non-empty delta: {events:?}"
    );
    assert!(events
        .iter()
        .any(|event| matches!(event, AiStreamEvent::CitationMetadata { .. })));
    assert!(events
        .iter()
        .any(|event| matches!(event, AiStreamEvent::Completed { .. })));
}
