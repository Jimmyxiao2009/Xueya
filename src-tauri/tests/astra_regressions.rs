use std::sync::Arc;
use verruckt_lib::db::repo::{courses, sources};
use verruckt_lib::domain::{Diagnosis, DiagnosisKind, SourceKind, SourceStatus};
use verruckt_lib::secrets::keyring_store::InMemorySecretStore;
use verruckt_lib::services::adaptation_service::AdaptationService;
use verruckt_lib::services::course_service::{CourseService, RouteRequest};
use verruckt_lib::services::ingestion_service::{IngestInput, IngestionService};
use verruckt_lib::state::{AppPaths, AppState};

async fn state() -> (AppState, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new_with_secrets(
        AppPaths::new(dir.path()),
        Arc::new(InMemorySecretStore::new()),
    )
    .await
    .unwrap();
    (state, dir)
}
fn input() -> IngestInput {
    IngestInput { filename:"audit.txt".into(), bytes:b"Projective geometry studies incidence. Points and lines preserve incidence under projective transformations.".to_vec(), kind:SourceKind::File,title:None,uri:None,mime_type:Some("text/plain".into()) }
}

#[test]
fn redaction_accepts_unicode_boundaries() {
    let input = format!("Bearer {}", "中".repeat(200));
    assert!(std::panic::catch_unwind(|| verruckt_lib::error::redact(&input)).is_ok());
}

#[tokio::test]
async fn failed_first_index_is_repairable_by_reimport() {
    let (s, _dir) = state().await;
    sqlx::query("CREATE TRIGGER audit_fail BEFORE INSERT ON embeddings BEGIN SELECT RAISE(FAIL, 'audit injected storage failure'); END;").execute(&s.pool).await.unwrap();
    let result = IngestionService::new(&s).ingest_bytes(input(), None).await;
    assert!(
        result.is_err(),
        "failure injection must reach production vector persistence"
    );
    let all = sources::list_sources(&s.pool).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].status, SourceStatus::Failed);
    assert!(
        verruckt_lib::knowledge::embedding_cache::source_index_needs_reindex(&s.pool, all[0].id)
            .await
            .unwrap()
    );
    sqlx::query("DROP TRIGGER audit_fail")
        .execute(&s.pool)
        .await
        .unwrap();
    let again = IngestionService::new(&s)
        .ingest_bytes(input(), None)
        .await
        .unwrap();
    assert_eq!(again.id, all[0].id);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM embeddings")
        .fetch_one(&s.pool)
        .await
        .unwrap();
    assert!(count > 0);
}

#[tokio::test]
async fn populated_source_deletes_successfully() {
    let (s, _dir) = state().await;
    let source = IngestionService::new(&s)
        .ingest_bytes(input(), None)
        .await
        .unwrap();
    let result = sources::delete_source(&s.pool, source.id).await;
    assert!(result.is_ok(), "{result:?}");
    assert!(sources::get_source(&s.pool, source.id).await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn parallel_diagnoses_insert_one_review() {
    let (s, _dir) = state().await;
    let course = CourseService::new(&s)
        .create_course(RouteRequest::new("Geometry"), None)
        .await
        .unwrap();
    let nodes = courses::list_nodes(&s.pool, course.id).await.unwrap();
    let node = nodes[0].id;
    let before = nodes.len();
    let barrier = Arc::new(tokio::sync::Barrier::new(8));
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let s = s.clone();
        let barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            let diagnosis = Diagnosis::new(
                DiagnosisKind::IncorrectAnswer,
                Vec::new(),
                0.9,
                0.9,
                "same audit diagnosis",
            );
            AdaptationService::new(&s)
                .apply(course.id, node, &diagnosis)
                .await
        }));
    }
    let mut errors = 0;
    for task in tasks {
        if task.await.unwrap().is_err() {
            errors += 1;
        }
    }
    let after = courses::list_nodes(&s.pool, course.id).await.unwrap();
    assert_eq!(after.len(), before + 1);
    assert_eq!(errors, 0);
}

#[tokio::test]
async fn restored_blob_rebuilds_documents_and_chunks() {
    let (s, _dir) = state().await;
    let source = IngestionService::new(&s)
        .ingest_bytes(input(), None)
        .await
        .unwrap();
    let (other, _otherdir) = state().await;
    let adapters = verruckt_lib::sync::adapters::adapters();
    for kind in ["source", "source_revision"] {
        let table = if kind == "source" {
            "sources"
        } else {
            "source_revisions"
        };
        let ids: Vec<uuid::Uuid> = sqlx::query_scalar(&format!("SELECT id FROM {table}"))
            .fetch_all(&s.pool)
            .await
            .unwrap();
        let adapter = adapters.iter().find(|a| a.entity_type() == kind).unwrap();
        for id in ids {
            let payload = adapter
                .load(&s.pool, &hex::encode(id.as_bytes()))
                .await
                .unwrap()
                .unwrap();
            let mut tx = other.pool.begin().await.unwrap();
            adapter
                .apply(&mut tx, &hex::encode(id.as_bytes()), &payload)
                .await
                .unwrap();
            tx.commit().await.unwrap();
        }
    }
    other.blobs.put_bytes(&input().bytes).await.unwrap();
    IngestionService::new(&other)
        .reindex_source(source.id, None)
        .await
        .unwrap();
    assert!(!sources::list_documents_by_source(&other.pool, source.id)
        .await
        .unwrap()
        .is_empty());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chunks")
        .fetch_one(&other.pool)
        .await
        .unwrap();
    assert!(count > 0);
}

#[tokio::test]
async fn node_fallback_stays_within_route_sources() {
    let (s, _dir) = state().await;
    let route_a = CourseService::new(&s)
        .create_course(RouteRequest::new("Projective geometry"), None)
        .await
        .unwrap();
    let nodes = courses::list_nodes(&s.pool, route_a.id).await.unwrap();
    let source = IngestionService::new(&s)
        .ingest_bytes(input(), None)
        .await
        .unwrap();
    let route_b = CourseService::new(&s)
        .create_course(
            RouteRequest {
                source_ids: vec![source.id],
                ..RouteRequest::new("Projective geometry")
            },
            None,
        )
        .await
        .unwrap();
    assert_ne!(route_a.id, route_b.id);
    let evidence = verruckt_lib::services::knowledge_service::KnowledgeService::new(&s)
        .evidence_for_node(&nodes[0], 8)
        .await
        .unwrap();
    assert!(evidence.is_empty());
}

#[tokio::test]
async fn demo_reset_preserves_ordinary_routes() {
    let (s, _dir) = state().await;
    let course = CourseService::new(&s)
        .create_course(RouteRequest::new("射影几何"), None)
        .await
        .unwrap();
    sqlx::query("UPDATE courses SET metadata_json = ? WHERE id = ?")
        .bind(r#"{"learningObjectives":["Explain the competitionDemo example"],"sourceIds":[]}"#)
        .bind(course.id)
        .execute(&s.pool)
        .await
        .unwrap();
    verruckt_lib::ipc::demo::seed_competition_demo(&s)
        .await
        .unwrap();
    assert!(courses::get_course(&s.pool, course.id).await.is_ok());
}

#[tokio::test]
async fn route_creation_rolls_back_all_rows_when_lesson_write_fails() {
    let (s, _dir) = state().await;
    sqlx::query("CREATE TRIGGER fail_lesson BEFORE INSERT ON lesson_steps BEGIN SELECT RAISE(FAIL, 'injected failure'); END").execute(&s.pool).await.unwrap();
    assert!(CourseService::new(&s)
        .create_course(RouteRequest::new("Geometry"), None)
        .await
        .is_err());
    for table in [
        "courses",
        "course_nodes",
        "knowledge_points",
        "lesson_steps",
        "quiz_questions",
        "adaptation_events",
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&s.pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "partial row persisted in {table}");
    }
}

#[tokio::test]
async fn quiz_failure_rolls_back_mastery_and_retry_is_idempotent() {
    use verruckt_lib::services::quiz_service::QuizService;
    let (s, _dir) = state().await;
    let course = CourseService::new(&s)
        .create_course(RouteRequest::new("Geometry"), None)
        .await
        .unwrap();
    let node = courses::list_nodes(&s.pool, course.id).await.unwrap()[0].clone();
    let questions = QuizService::new(&s)
        .questions_for_node(node.id)
        .await
        .unwrap();
    let question = questions
        .iter()
        .find(|q| q.kind() == verruckt_lib::domain::QuizKind::Single)
        .unwrap();
    sqlx::query("CREATE TRIGGER fail_attempt BEFORE INSERT ON quiz_attempts BEGIN SELECT RAISE(FAIL, 'injected failure'); END").execute(&s.pool).await.unwrap();
    assert!(QuizService::new(&s)
        .submit(question.id, question.expected(), None)
        .await
        .is_err());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM learner_states")
        .fetch_one(&s.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(
        courses::get_node(&s.pool, node.id)
            .await
            .unwrap()
            .confidence,
        node.confidence
    );
    sqlx::query("DROP TRIGGER fail_attempt")
        .execute(&s.pool)
        .await
        .unwrap();
    let first = QuizService::new(&s)
        .submit(question.id, question.expected(), None)
        .await
        .unwrap();
    let repeated = QuizService::new(&s)
        .submit(question.id, question.expected(), None)
        .await
        .unwrap();
    assert_eq!(first.mastery, repeated.mastery);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM quiz_attempts")
        .fetch_one(&s.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn fts_tracks_chunk_update_and_delete() {
    let (s, _dir) = state().await;
    let source = IngestionService::new(&s)
        .ingest_bytes(input(), None)
        .await
        .unwrap();
    sqlx::query("UPDATE chunks SET text='uniquereplacementword' WHERE document_id IN (SELECT id FROM documents WHERE source_id=?)").bind(source.id).execute(&s.pool).await.unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM chunk_fts WHERE chunk_fts MATCH 'uniquereplacementword'",
    )
    .fetch_one(&s.pool)
    .await
    .unwrap();
    assert!(count > 0);
    sources::delete_source(&s.pool, source.id).await.unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM chunk_fts WHERE chunk_fts MATCH 'uniquereplacementword'",
    )
    .fetch_one(&s.pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn docx_rejects_oversized_decompressed_xml() {
    use std::io::{Cursor, Write};
    use verruckt_lib::ingest::parser::{docx::DocxParser, DocumentParser};
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    archive
        .start_file(
            "word/document.xml",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
    let block = [b'x'; 65536];
    for _ in 0..257 {
        archive.write_all(&block).unwrap();
    }
    let bytes = archive.finish().unwrap().into_inner();
    let error = DocxParser.parse("large.docx", None, &bytes).unwrap_err();
    assert!(error.to_string().contains("size limit"));
}

#[test]
fn mapped_ipv6_cannot_bypass_private_address_filter() {
    use verruckt_lib::ingest::url_security::is_forbidden_ip;
    for ip in [
        "::ffff:127.0.0.1",
        "::ffff:10.0.0.1",
        "::ffff:169.254.169.254",
    ] {
        assert!(is_forbidden_ip(ip.parse().unwrap()), "{ip}");
    }
    assert!(!is_forbidden_ip("::ffff:93.184.216.34".parse().unwrap()));
}
