use std::sync::Arc;
use tempfile::TempDir;
use verruckt_lib::domain::CourseNodeStatus;
use verruckt_lib::ipc::demo::seed_competition_demo;
use verruckt_lib::secrets::keyring_store::{InMemorySecretStore, SecretStore};
use verruckt_lib::services::course_service::CourseService;
use verruckt_lib::services::source_service;
use verruckt_lib::state::{AppPaths, AppState};

async fn make_state(secrets: Arc<dyn SecretStore>, dir: &TempDir) -> AppState {
    AppState::new_with_secrets(AppPaths::new(dir.path()), secrets)
        .await
        .expect("app state")
}

#[tokio::test]
async fn competition_demo_seed_rebuilds_a_real_grounded_route() {
    let dir = TempDir::new().expect("tempdir");
    let secrets: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let state = make_state(secrets, &dir).await;

    let first_id = seed_competition_demo(&state)
        .await
        .expect("first demo seed");
    let second_id = seed_competition_demo(&state)
        .await
        .expect("second demo seed");

    let courses = CourseService::new(&state)
        .list_courses()
        .await
        .expect("list courses");
    let demo_courses: Vec<_> = courses
        .iter()
        .filter(|course| course.topic == "射影几何")
        .collect();
    assert_eq!(demo_courses.len(), 1, "reset must leave one demo route");
    assert_ne!(first_id, second_id, "reset must rebuild the route");

    let sources = source_service::list_sources(&state.pool)
        .await
        .expect("list sources");
    assert!(sources.len() >= 2, "demo route must use real local sources");
    assert!(
        sources
            .iter()
            .any(|source| source.title == "射影平面与无穷远点"),
        "missing fixture source: {sources:?}"
    );

    let (course, nodes, _) = CourseService::new(&state)
        .get_course_bundle(second_id)
        .await
        .expect("course bundle");
    assert!(
        course.metadata_json.contains("competitionDemo"),
        "demo marker missing: {}",
        course.metadata_json
    );
    assert!(
        course.metadata_json.contains("sourceIds"),
        "route must keep source grounding: {}",
        course.metadata_json
    );
    assert!(nodes.len() >= 2, "demo route needs at least two nodes");
    assert_eq!(nodes[0].status, CourseNodeStatus::Completed);
    assert_eq!(nodes[1].status, CourseNodeStatus::Available);
}
