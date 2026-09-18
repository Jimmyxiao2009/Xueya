//! Repeated competition golden-path validation.
//!
//! Each iteration uses a completely independent temporary AppState, runs the
//! same real services the desktop UI calls, and verifies that the review node
//! survives a database close/reopen. The test fails on the first iteration
//! whose real path cannot complete, without retrying the whole run to hide it.

use std::sync::Arc;
use tempfile::TempDir;
use verruckt_lib::db::repo::courses as course_repo;
use verruckt_lib::domain::QuizKind;
use verruckt_lib::ipc::demo::seed_competition_demo;
use verruckt_lib::secrets::keyring_store::{InMemorySecretStore, SecretStore};
use verruckt_lib::services::learning_service::LearningService;
use verruckt_lib::services::quiz_service::QuizService;
use verruckt_lib::services::tutor_service::TutorService;
use verruckt_lib::state::{AppPaths, AppState};

async fn make_state(secrets: Arc<dyn SecretStore>, dir: &TempDir) -> AppState {
    AppState::new_with_secrets(AppPaths::new(dir.path()), secrets)
        .await
        .expect("app state")
}

async fn run_iteration(iteration: usize) {
    let dir = TempDir::new().expect("tempdir");
    let secrets: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let state = make_state(secrets.clone(), &dir).await;

    // 01-06: seed the real local fixture route and open the current node.
    let course_id = seed_competition_demo(&state)
        .await
        .unwrap_or_else(|error| panic!("iteration {iteration}: demo seed failed: {error}"));
    let (_, nodes, _) = verruckt_lib::services::course_service::CourseService::new(&state)
        .get_course_bundle(course_id)
        .await
        .expect("course bundle");
    assert!(
        nodes.len() >= 2,
        "iteration {iteration}: demo route needs two nodes"
    );
    let node_id = nodes[1].id;

    // 07-10: start the real learning session and stream a grounded tutor turn.
    let started = LearningService::new(&state)
        .start(node_id, None)
        .await
        .unwrap_or_else(|error| panic!("iteration {iteration}: learning start failed: {error}"));
    let turn = TutorService::new(&state)
        .turn(started.session.id, "平行线在射影平面里相交于哪里？")
        .await
        .unwrap_or_else(|error| panic!("iteration {iteration}: tutor turn failed: {error}"));
    assert!(
        !turn.message.trim().is_empty(),
        "iteration {iteration}: tutor message is empty"
    );
    assert_eq!(
        turn.grounding, "source_grounded",
        "iteration {iteration}: expected source-grounded answer"
    );
    assert!(
        !turn.citations.is_empty(),
        "iteration {iteration}: expected at least one real citation"
    );
    assert!(
        turn.citations
            .iter()
            .all(|citation| !citation.source_title.trim().is_empty()),
        "iteration {iteration}: citation without source title"
    );

    // 13-18: submit a deliberately wrong objective answer and force adaptation.
    let questions = QuizService::new(&state)
        .questions_for_node(node_id)
        .await
        .expect("questions");
    let question = questions
        .iter()
        .find(|question| question.kind() == QuizKind::Single)
        .unwrap_or_else(|| panic!("iteration {iteration}: no single-choice quiz question"));
    let expected = question.expected();
    let wrong = question
        .options()
        .into_iter()
        .find(|option| !expected.contains(option))
        .unwrap_or_else(|| {
            panic!("iteration {iteration}: single-choice question has no distractor")
        });
    let outcome = QuizService::new(&state)
        .submit(question.id, vec![wrong], Some(started.session.id))
        .await
        .unwrap_or_else(|error| panic!("iteration {iteration}: quiz submit failed: {error}"));
    assert!(
        !outcome.grade.correct,
        "iteration {iteration}: wrong answer was graded correct"
    );
    assert!(
        outcome.inserted_node_id.is_some(),
        "iteration {iteration}: wrong answer did not insert a review node"
    );

    drop(state);

    // 19-21: close the database, reopen the same profile, and verify persistence.
    let reopened = make_state(secrets, &dir).await;
    let nodes_after_restart = course_repo::list_nodes(&reopened.pool, course_id)
        .await
        .unwrap_or_else(|error| panic!("iteration {iteration}: reopen failed: {error}"));
    assert!(
        nodes_after_restart.len() > nodes.len(),
        "iteration {iteration}: review node did not survive restart"
    );
    assert!(
        nodes_after_restart
            .iter()
            .any(|node| node.id == outcome.inserted_node_id.expect("inserted id")),
        "iteration {iteration}: inserted review node is missing after restart"
    );

    println!("GOLDEN_PATH iteration {iteration}/10 PASS");
}

#[tokio::test]
async fn golden_path_ten_consecutive_iterations() {
    for iteration in 1..=10 {
        run_iteration(iteration).await;
    }
}
