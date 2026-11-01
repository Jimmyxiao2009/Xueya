//! Regression coverage for the supplemental audit findings.
//! Synthetic data only; no production credentials or external services.
use std::{collections::HashSet, sync::Arc};
use uuid::Uuid;
use verruckt_lib::{
    domain::SourceKind,
    knowledge::{
        embedding::upsert_embedding, vector_index::VectorIndex,
        verruckt_vector::VerrucktVectorStoreAdapter,
    },
    secrets::{
        encrypted_store::EncryptedSecretStore,
        keyring_store::{InMemorySecretStore, SecretStore},
    },
    services::ingestion_service::{IngestInput, IngestionService},
    state::{AppPaths, AppState, VectorBackend},
};

async fn fixture() -> (tempfile::TempDir, AppState, Vec<Uuid>, Uuid) {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new_with_secrets_and_backend(
        AppPaths::new(dir.path()),
        Arc::new(InMemorySecretStore::new()),
        VectorBackend::Sqlite,
    )
    .await
    .unwrap();
    for text in [
        "# Route A\nA synthetic astronomy source.",
        "# Route B\nA synthetic geometry source.",
    ] {
        IngestionService::new(&state)
            .ingest_bytes(
                IngestInput {
                    filename: "audit.md".into(),
                    bytes: text.as_bytes().to_vec(),
                    kind: SourceKind::File,
                    title: None,
                    uri: None,
                    mime_type: Some("text/markdown".into()),
                },
                None,
            )
            .await
            .unwrap();
    }
    let ids = sqlx::query_scalar::<_, Uuid>("SELECT id FROM chunks ORDER BY id")
        .fetch_all(&state.pool)
        .await
        .unwrap();
    assert_eq!(ids.len(), 2);
    let provider = sqlx::query_scalar::<_, Uuid>("SELECT provider_id FROM embeddings LIMIT 1")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    (dir, state, ids, provider)
}

#[tokio::test]
async fn audit_route_filter_must_search_within_allowed_set() {
    let (dir, state, ids, provider) = fixture().await;
    let adapter = VerrucktVectorStoreAdapter::new(state.pool.clone(), dir.path())
        .await
        .unwrap();
    adapter
        .upsert(ids[0], provider, "audit", &[0.8, 0.6])
        .await
        .unwrap();
    adapter
        .upsert(ids[1], provider, "audit", &[1.0, 0.0])
        .await
        .unwrap();
    let allowed = HashSet::from([ids[0]]);
    let hits = adapter
        .search_filtered(&[1.0, 0.0], provider, "audit", Some(&allowed), 1)
        .await
        .unwrap();
    assert_eq!(
        hits.iter().map(|h| h.chunk_id).collect::<Vec<_>>(),
        vec![ids[0]],
        "another route's stronger hit must not crowd out the only allowed result"
    );
}

#[tokio::test]
async fn audit_rebuild_must_refresh_same_count_changed_vectors() {
    let (dir, state, ids, provider) = fixture().await;
    let adapter = VerrucktVectorStoreAdapter::new(state.pool.clone(), dir.path())
        .await
        .unwrap();
    adapter
        .upsert(ids[0], provider, "audit", &[1.0, 0.0])
        .await
        .unwrap();
    adapter
        .upsert(ids[1], provider, "audit", &[0.0, 1.0])
        .await
        .unwrap();
    adapter.flush().await.unwrap();
    drop(adapter);
    // Simulate authoritative SQLite updates whose derived writes were interrupted,
    // or a run with the default SQLite backend before switching back to Verruckt.
    upsert_embedding(&state.pool, ids[0], provider, "audit", &[0.0, 1.0])
        .await
        .unwrap();
    upsert_embedding(&state.pool, ids[1], provider, "audit", &[1.0, 0.0])
        .await
        .unwrap();
    let reopened = VerrucktVectorStoreAdapter::new(state.pool.clone(), dir.path())
        .await
        .unwrap();
    reopened.rebuild().await.unwrap();
    let hits = reopened
        .search(&[1.0, 0.0], provider, "audit", 1)
        .await
        .unwrap();
    assert_eq!(
        hits[0].chunk_id, ids[1],
        "rebuild must reflect SQLite contents, not just row count"
    );
}

#[tokio::test]
async fn audit_rebuild_must_not_resurrect_deleted_space() {
    let (dir, state, ids, provider) = fixture().await;
    let adapter = VerrucktVectorStoreAdapter::new(state.pool.clone(), dir.path())
        .await
        .unwrap();
    adapter
        .upsert(ids[0], provider, "audit", &[1.0, 0.0])
        .await
        .unwrap();
    adapter.flush().await.unwrap();
    drop(adapter);
    sqlx::query("DELETE FROM embeddings WHERE model = 'audit'")
        .execute(&state.pool)
        .await
        .unwrap();
    let reopened = VerrucktVectorStoreAdapter::new(state.pool.clone(), dir.path())
        .await
        .unwrap();
    reopened.rebuild().await.unwrap();
    let hits = reopened
        .search(&[1.0, 0.0], provider, "audit", 1)
        .await
        .unwrap();
    assert!(
        hits.is_empty(),
        "a space absent from authoritative SQLite must not return old vectors"
    );
}

#[tokio::test]
async fn audit_corrupt_vault_must_report_error_and_preserve_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let store = EncryptedSecretStore::new(dir.path()).unwrap();
    store
        .set_secret("synthetic-key", "synthetic-value")
        .await
        .unwrap();
    drop(store);
    let path = dir.path().join("secure/vault.enc");
    let mut bytes = std::fs::read(&path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    std::fs::write(&path, bytes).unwrap();
    assert!(
        EncryptedSecretStore::new(dir.path()).is_err(),
        "authentication failure must not silently initialize an empty writable vault"
    );
}

#[tokio::test]
async fn audit_failed_secret_write_must_not_change_visible_value() {
    let dir = tempfile::tempdir().unwrap();
    let store = EncryptedSecretStore::new(dir.path()).unwrap();
    store
        .set_secret("synthetic-key", "old-value")
        .await
        .unwrap();
    // A deterministic filesystem error, without relying on host ACL privileges.
    std::fs::create_dir(dir.path().join("secure/vault.tmp")).unwrap();
    assert!(store
        .set_secret("synthetic-key", "new-value")
        .await
        .is_err());
    assert_eq!(
        store.get_secret("synthetic-key").await.unwrap(),
        "old-value",
        "failed save must not expose an unpersisted credential as current"
    );
}

#[test]
fn audit_engine_runtime_checkpoint_option_must_be_honored() {
    use verruckt_vector::{
        EngineOptions, IndexConfig, Metadata, Metric, VectorId, VectorIndex as Engine,
    };
    let dir = tempfile::tempdir().unwrap();
    drop(Engine::create(dir.path(), IndexConfig::new(2, Metric::Cosine)).unwrap());
    let index = Engine::open_with(
        dir.path(),
        &EngineOptions {
            wal_auto_checkpoint_bytes: Some(1),
            sync_wal_per_op: true,
            ..EngineOptions::default()
        },
    )
    .unwrap();
    let before = index.stats().unwrap().generation;
    index
        .insert(VectorId(1), &[1.0, 0.0], Metadata::empty())
        .unwrap();
    assert!(
        index.stats().unwrap().generation > before,
        "a one-byte runtime WAL threshold must checkpoint after the first record"
    );
}
