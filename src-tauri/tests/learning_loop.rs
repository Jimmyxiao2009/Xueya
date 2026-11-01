//! End-to-end checks for the learning loop that the prototype faked:
//! create a route -> open a node -> answer a question -> the route adapts.

use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;
use verruckt_lib::bootstrap;
use verruckt_lib::db::repo::{
    courses as course_repo, learning as learning_repo, learning_loop as loop_repo,
};
use verruckt_lib::domain::{AgentActionKind, CourseNodeStatus, NodeKind, QuizKind};
use verruckt_lib::secrets::keyring_store::InMemorySecretStore;
use verruckt_lib::services::course_service::{CourseService, RouteRequest};
use verruckt_lib::services::learning_service::LearningService;
use verruckt_lib::services::quiz_service::QuizService;
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

async fn create_route(state: &AppState, topic: &str) -> Uuid {
    CourseService::new(state)
        .create_course(RouteRequest::new(topic), None)
        .await
        .expect("route")
        .id
}

#[tokio::test]
async fn first_launch_ships_both_prototype_routes() {
    let (state, _dir) = test_state().await;
    bootstrap::seed_demo_routes(&state).await.expect("seed");

    let courses = course_repo::list_courses(&state.pool).await.expect("list");
    assert_eq!(courses.len(), 2);
    assert_eq!(courses[0].topic, "射影几何");
    assert_eq!(courses[1].topic, "线性代数");
    assert!(courses.iter().all(|course| course.seeded == 1));
    assert_eq!(courses[0].kind, "geometry");
    assert_eq!(courses[1].kind, "algebra");

    // Seeding is idempotent: a second launch must not duplicate the demo data.
    bootstrap::seed_demo_routes(&state)
        .await
        .expect("seed again");
    assert_eq!(
        course_repo::count_courses(&state.pool)
            .await
            .expect("count"),
        2
    );
}

#[tokio::test]
async fn seeded_route_opens_with_one_completed_node() {
    let (state, _dir) = test_state().await;
    bootstrap::seed_demo_routes(&state).await.expect("seed");
    let courses = course_repo::list_courses(&state.pool).await.expect("list");
    let nodes = course_repo::list_nodes(&state.pool, courses[0].id)
        .await
        .expect("nodes");

    assert_eq!(nodes[0].status, CourseNodeStatus::Completed);
    assert_eq!(nodes[1].status, CourseNodeStatus::Available);
    assert_eq!(nodes[2].status, CourseNodeStatus::Locked);

    let progress = course_repo::progress_for_course(&state.pool, courses[0].id)
        .await
        .expect("progress");
    assert_eq!(progress.total, 6);
    assert_eq!(progress.completed, 1);
    assert_eq!(progress.current_node_id, Some(nodes[1].id));
}

#[tokio::test]
async fn a_new_route_is_a_full_plan_not_a_stub() {
    let (state, _dir) = test_state().await;
    let course_id = create_route(&state, "线性代数").await;

    let nodes = course_repo::list_nodes(&state.pool, course_id)
        .await
        .expect("nodes");
    assert_eq!(nodes.len(), 6);
    assert_eq!(nodes[0].kind, NodeKind::Lesson.as_str());
    assert_eq!(nodes[2].kind, NodeKind::Review.as_str());
    assert_eq!(nodes[4].kind, NodeKind::Quiz.as_str());
    assert_eq!(nodes[0].status, CourseNodeStatus::Available);
    assert!(nodes
        .iter()
        .skip(1)
        .all(|node| node.status == CourseNodeStatus::Locked));
    assert!(nodes.iter().all(|node| !node.generated_by.is_empty()));

    // A linear chain of prerequisites, so unlocking is deterministic.
    let dependencies = course_repo::list_dependencies(&state.pool, course_id)
        .await
        .expect("deps");
    assert_eq!(dependencies.len(), nodes.len() - 1);
    for pair in nodes.windows(2) {
        assert!(dependencies
            .iter()
            .any(|dep| dep.node_id == pair[1].id && dep.depends_on_node_id == pair[0].id));
    }

    // Every node carries its lesson plan and its question set.
    for node in &nodes {
        let steps = loop_repo::list_lesson_steps(&state.pool, node.id)
            .await
            .expect("steps");
        assert!(!steps.is_empty(), "node {} has no lesson steps", node.title);
        let questions = loop_repo::list_quiz_questions(&state.pool, node.id)
            .await
            .expect("questions");
        assert!(
            !questions.is_empty(),
            "node {} has no questions",
            node.title
        );
    }

    let points = loop_repo::list_knowledge_points(&state.pool, course_id)
        .await
        .expect("points");
    assert_eq!(points.len(), nodes.len());
}

#[tokio::test]
async fn opening_a_node_returns_a_usable_lesson_without_any_api_key() {
    let (state, _dir) = test_state().await;
    let course_id = create_route(&state, "射影几何").await;
    let nodes = course_repo::list_nodes(&state.pool, course_id)
        .await
        .expect("nodes");

    let prepared = CourseService::new(&state)
        .prepare_node(nodes[0].id, None)
        .await
        .expect("lesson");

    assert!(!prepared.lesson.title.is_empty());
    assert!(!prepared.lesson.explanation.is_empty());
    assert!(!prepared.lesson.key_points.is_empty());
    assert!(!prepared.steps.is_empty());
    assert!(!prepared.questions.is_empty());
    assert_eq!(prepared.generated_by, "deterministic");
    assert!(prepared.mastery < 0.001);
}

#[tokio::test]
async fn a_wrong_answer_updates_the_learner_model_and_inserts_practice() {
    let (state, _dir) = test_state().await;
    let course_id = create_route(&state, "射影几何").await;
    let nodes = course_repo::list_nodes(&state.pool, course_id)
        .await
        .expect("nodes");
    let node = nodes[0].clone();

    let questions = loop_repo::list_quiz_questions(&state.pool, node.id)
        .await
        .expect("questions");
    let objective = questions
        .iter()
        .find(|question| question.kind() == QuizKind::Single)
        .expect("a locally gradable question")
        .clone();
    let expected = objective.expected();
    let wrong = objective
        .options()
        .into_iter()
        .find(|option| !expected.contains(option))
        .expect("a distractor");

    let outcome = QuizService::new(&state)
        .submit(objective.id, vec![wrong], None)
        .await
        .expect("submit");

    assert!(!outcome.grade.correct);
    assert!(outcome.grade.graded);
    assert!(outcome.mastery < 0.5);
    assert!(outcome.next_review.is_some());
    assert!(outcome.adaptation.is_some());

    // The prototype inserted a "加练 · …" node in front of the learner; so do we.
    let inserted = outcome.inserted_node_id.expect("review node");
    let review = course_repo::get_node(&state.pool, inserted)
        .await
        .expect("node");
    assert_eq!(review.kind, NodeKind::Review.as_str());
    assert!(review.title.starts_with("加练 · "));
    assert_eq!(review.order_hint, node.order_hint + 1);

    let states = loop_repo::list_learner_states(&state.pool, course_id)
        .await
        .expect("states");
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].mistakes, 1);
    assert_eq!(states[0].attempts, 1);
    assert!(states[0].next_review.is_some());

    // Answering the same wrong thing twice must not spam the roadmap.
    let again = QuizService::new(&state)
        .submit(objective.id, vec!["这不是一个选项".to_owned()], None)
        .await
        .expect("submit again");
    assert!(again.inserted_node_id.is_none());
    let adaptations = loop_repo::list_adaptations(&state.pool, course_id, 50)
        .await
        .expect("adaptations");
    assert_eq!(
        adaptations
            .iter()
            .filter(|event| event.kind == "review_inserted")
            .count(),
        1
    );
}

#[tokio::test]
async fn a_free_text_answer_is_gradable_before_any_source_is_imported() {
    let (state, _dir) = test_state().await;
    let course_id = create_route(&state, "射影几何").await;
    let nodes = course_repo::list_nodes(&state.pool, course_id)
        .await
        .expect("nodes");
    let node = nodes[0].clone();

    // A route planned from nothing has no evidence chunks yet. Grading must
    // still work: the lesson's own explanation is enough to judge an answer.
    let started = LearningService::new(&state)
        .start(node.id, None)
        .await
        .expect("session");
    let response = LearningService::new(&state)
        .respond(
            started.session.id,
            "因为方向相同，它们交在同一个无穷远点上",
            None,
        )
        .await
        .expect("evaluation");

    assert!(response.evaluation.correctness >= 0.0);
    assert!(response.evaluation.correctness <= 1.0);
    assert!(!response.evaluation.feedback.trim().is_empty());
    assert_eq!(response.attempts, 1);

    // The attempt is persisted, and it moves the learner model.
    let attempts = learning_repo::list_attempts_for_node(&state.pool, node.id)
        .await
        .expect("attempts");
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].session_id, started.session.id);
}

#[tokio::test]
async fn a_confused_learner_gets_targeted_practice_on_the_roadmap() {
    let (state, _dir) = test_state().await;
    let course_id = create_route(&state, "射影几何").await;
    let nodes = course_repo::list_nodes(&state.pool, course_id)
        .await
        .expect("nodes");
    let node = nodes[0].clone();

    let started = LearningService::new(&state)
        .start(node.id, None)
        .await
        .expect("session");

    // The offline provider names a misconception for this phrasing, so the
    // whole diagnose -> adapt step is exercisable with no API key at all.
    let turn = TutorService::new(&state)
        .turn(started.session.id, "我还是不懂为什么要这样定义")
        .await
        .expect("tutor turn");
    assert!(turn.diagnosis.is_some());

    let adaptation = turn.adaptation.expect("an adaptation");
    assert_eq!(adaptation.kind, "review_inserted");

    let after = course_repo::list_nodes(&state.pool, course_id)
        .await
        .expect("nodes after");
    assert_eq!(after.len(), nodes.len() + 1);
    let review = after
        .iter()
        .find(|candidate| candidate.kind == NodeKind::Review.as_str())
        .expect("a review node");
    assert!(review.title.starts_with("加练 · "));
    assert_eq!(review.order_hint, node.order_hint + 1);

    // A second confused turn must not stack another node on the same place.
    TutorService::new(&state)
        .turn(started.session.id, "还是不明白")
        .await
        .expect("second turn");
    let after_twice = course_repo::list_nodes(&state.pool, course_id)
        .await
        .expect("nodes after twice");
    assert_eq!(after_twice.len(), after.len());

    // The conversation itself is persisted, both sides of it.
    let messages = loop_repo::list_messages(&state.pool, started.session.id)
        .await
        .expect("messages");
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[0].0, "user");
    assert_eq!(messages[1].0, "assistant");
}

#[tokio::test]
async fn a_correct_answer_does_not_touch_the_roadmap() {
    let (state, _dir) = test_state().await;
    let course_id = create_route(&state, "射影几何").await;
    let nodes = course_repo::list_nodes(&state.pool, course_id)
        .await
        .expect("nodes");

    let questions = loop_repo::list_quiz_questions(&state.pool, nodes[0].id)
        .await
        .expect("questions");
    let objective = questions
        .iter()
        .find(|question| question.kind() == QuizKind::Single)
        .expect("question")
        .clone();

    let outcome = QuizService::new(&state)
        .submit(objective.id, objective.expected(), None)
        .await
        .expect("submit");

    assert!(outcome.grade.correct);
    assert!(outcome.inserted_node_id.is_none());
    assert!(outcome.adaptation.is_none());
    // One right answer nudges mastery, it does not max it out.
    assert!(outcome.mastery > 0.2 && outcome.mastery < 0.6);

    let after = course_repo::list_nodes(&state.pool, course_id)
        .await
        .expect("nodes");
    assert_eq!(after.len(), nodes.len());
}

#[tokio::test]
async fn finishing_a_node_unlocks_the_next_one() {
    let (state, _dir) = test_state().await;
    let course_id = create_route(&state, "射影几何").await;
    let nodes = course_repo::list_nodes(&state.pool, course_id)
        .await
        .expect("nodes");

    let course = CourseService::new(&state)
        .complete_node(nodes[0].id)
        .await
        .expect("complete");
    assert_eq!(course.current_node_id, Some(nodes[1].id));

    let after = course_repo::list_nodes(&state.pool, course_id)
        .await
        .expect("nodes");
    assert_eq!(after[0].status, CourseNodeStatus::Completed);
    assert_eq!(after[1].status, CourseNodeStatus::Available);
    assert_eq!(after[2].status, CourseNodeStatus::Locked);

    let progress = course_repo::progress_for_course(&state.pool, course_id)
        .await
        .expect("progress");
    assert!((progress.ratio() - 1.0 / 6.0).abs() < 1e-9);
}

#[tokio::test]
async fn depth_and_experience_change_the_plan() {
    let (state, _dir) = test_state().await;

    let mut deep = RouteRequest::new("射影几何");
    deep.desired_depth = Some("deep".to_owned());
    let deep_course = CourseService::new(&state)
        .create_course(deep, None)
        .await
        .expect("deep route");
    let deep_nodes = course_repo::list_nodes(&state.pool, deep_course.id)
        .await
        .expect("nodes");
    assert!(deep_nodes.len() > 6);

    let mut advanced = RouteRequest::new("线性代数");
    advanced.experience_level = Some("advanced".to_owned());
    let advanced_course = CourseService::new(&state)
        .create_course(advanced, None)
        .await
        .expect("advanced route");
    let advanced_nodes = course_repo::list_nodes(&state.pool, advanced_course.id)
        .await
        .expect("nodes");
    assert!(advanced_nodes.len() < 6);
    assert_eq!(advanced_nodes[0].status, CourseNodeStatus::Available);
}

#[tokio::test]
async fn deleting_a_route_removes_its_nodes() {
    let (state, _dir) = test_state().await;
    let course_id = create_route(&state, "射影几何").await;
    CourseService::new(&state)
        .delete_course(course_id)
        .await
        .expect("delete");
    assert!(course_repo::list_nodes(&state.pool, course_id)
        .await
        .expect("nodes")
        .is_empty());
    assert_eq!(
        course_repo::count_courses(&state.pool)
            .await
            .expect("count"),
        0
    );
}

#[tokio::test]
async fn every_question_kind_survives_the_migration() {
    let (state, _dir) = test_state().await;
    let course_id = create_route(&state, "射影几何").await;
    let nodes = course_repo::list_nodes(&state.pool, course_id)
        .await
        .expect("nodes");
    let questions = loop_repo::list_quiz_questions(&state.pool, nodes[0].id)
        .await
        .expect("questions");

    // The prototype's five widget types must all survive the migration.
    let kinds: Vec<String> = questions
        .iter()
        .map(|question| question.kind.clone())
        .collect();
    for expected in ["single", "multi", "fill", "short", "image"] {
        assert!(kinds.contains(&expected.to_owned()), "missing {expected}");
    }

    for question in &questions {
        let empty = QuizService::new(&state)
            .submit(question.id, Vec::new(), None)
            .await
            .expect("submit empty");
        assert!(!empty.grade.correct);
        assert!(!empty.grade.feedback.is_empty());
    }
}

#[tokio::test]
async fn a_weak_free_text_answer_goes_through_evaluation_to_adaptation() {
    let (state, _dir) = test_state().await;
    let course_id = create_route(&state, "射影几何").await;
    let nodes = course_repo::list_nodes(&state.pool, course_id)
        .await
        .expect("nodes");
    let node = nodes[0].clone();
    let started = LearningService::new(&state)
        .start(node.id, None)
        .await
        .expect("session");

    // The offline evaluator marks this as incorrect, which is the free-text
    // equivalent of a failed check question.
    let response = LearningService::new(&state)
        .respond(started.session.id, "不知道", None)
        .await
        .expect("evaluation");
    assert!(!response.diagnoses.is_empty());
    assert_eq!(
        response
            .adaptation
            .as_ref()
            .map(|event| event.kind.as_str()),
        Some("review_inserted")
    );
    let inserted = response.inserted_node_id.expect("review node id");

    let after = course_repo::list_nodes(&state.pool, course_id)
        .await
        .expect("nodes after");
    assert_eq!(after.len(), nodes.len() + 1);
    assert!(after
        .iter()
        .any(|candidate| candidate.id == inserted && candidate.kind == NodeKind::Review.as_str()));

    // A repeated weak answer must resolve to the same practice node.
    LearningService::new(&state)
        .respond(started.session.id, "还是不知道", None)
        .await
        .expect("second evaluation");
    let after_twice = course_repo::list_nodes(&state.pool, course_id)
        .await
        .expect("nodes after twice");
    assert_eq!(after_twice.len(), after.len());
    assert_eq!(
        after_twice
            .iter()
            .filter(|candidate| candidate.id == inserted)
            .count(),
        1
    );
}

#[tokio::test]
async fn an_adaptation_survives_a_database_reopen_and_stays_idempotent() {
    let (state, dir) = test_state().await;
    let course_id = create_route(&state, "射影几何").await;
    let nodes = course_repo::list_nodes(&state.pool, course_id)
        .await
        .expect("nodes");
    let started = LearningService::new(&state)
        .start(nodes[0].id, None)
        .await
        .expect("session");

    let turn = TutorService::new(&state)
        .turn(started.session.id, "我还是不懂为什么要这样定义")
        .await
        .expect("tutor turn");
    let review_id = turn.inserted_node_id.expect("inserted review node");
    assert_eq!(
        course_repo::list_nodes(&state.pool, course_id)
            .await
            .expect("nodes after")
            .len(),
        nodes.len() + 1
    );

    // Close the SQLite pool and rebuild the app state from the same directory:
    // the adaptation must be a database fact, not in-memory tutor state.
    state.pool.close().await;
    drop(state);
    let reopened = AppState::new_with_secrets(
        AppPaths::new(dir.path()),
        Arc::new(InMemorySecretStore::new()),
    )
    .await
    .expect("reopen app state");

    let after = course_repo::list_nodes(&reopened.pool, course_id)
        .await
        .expect("nodes after reopen");
    assert!(after
        .iter()
        .any(|node| node.id == review_id && node.kind == NodeKind::Review.as_str()));

    // The same diagnosis on the reopened database must resolve to the same
    // review node instead of inserting a second one.
    TutorService::new(&reopened)
        .turn(started.session.id, "还是不明白")
        .await
        .expect("second turn after reopen");
    let after_twice = course_repo::list_nodes(&reopened.pool, course_id)
        .await
        .expect("nodes after second turn");
    assert_eq!(after_twice.len(), after.len());
    assert_eq!(
        after_twice
            .iter()
            .filter(|node| node.id == review_id)
            .count(),
        1
    );
}

#[tokio::test]
async fn session_opening_is_agent_authored_and_persisted() {
    let (state, _dir) = test_state().await;
    let course_id = create_route(&state, "射影几何").await;
    let nodes = course_repo::list_nodes(&state.pool, course_id)
        .await
        .expect("nodes");
    let started = LearningService::new(&state)
        .start(nodes[0].id, None)
        .await
        .expect("session");

    let opening = TutorService::new(&state)
        .opening(started.session.id)
        .await
        .expect("agent opening");
    assert!(!opening.message.trim().is_empty());
    assert_eq!(opening.generated_by, "mock");
    assert!(
        opening.message.contains("先不急着记定义") || opening.message.contains('？'),
        "opening should be a concrete teaching move, got: {}",
        opening.message
    );

    let messages = loop_repo::list_messages(&state.pool, started.session.id)
        .await
        .expect("messages");
    assert!(
        messages
            .iter()
            .any(|(role, content)| role == "assistant" && content == &opening.message),
        "opening must be persisted as the first assistant message"
    );

    let turns = TutorService::new(&state)
        .turns(started.session.id)
        .await
        .expect("turns");
    assert_eq!(turns.len(), 1);
    assert!(turns[0].learner_response.is_none());
    assert_eq!(turns[0].agent_action.kind, AgentActionKind::Ask);

}
#[tokio::test]
async fn tutor_turns_pair_learner_response_with_agent_action() {
    let (state, _dir) = test_state().await;
    let course_id = create_route(&state, "线性代数").await;
    let nodes = course_repo::list_nodes(&state.pool, course_id)
        .await
        .expect("nodes");
    let started = LearningService::new(&state)
        .start(nodes[0].id, None)
        .await
        .expect("session");

    TutorService::new(&state)
        .opening(started.session.id)
        .await
        .expect("opening");
    TutorService::new(&state)
        .turn(started.session.id, "我不太懂特征值")
        .await
        .expect("tutor turn");

    let turns = TutorService::new(&state)
        .turns(started.session.id)
        .await
        .expect("turns");
    assert_eq!(turns.len(), 2);
    assert!(turns[0].learner_response.is_none());
    let second = &turns[1];
    assert_eq!(
        second.learner_response.as_ref().expect("learner response").text,
        "我不太懂特征值"
    );
    assert!(!second.agent_action.suggested_action.trim().is_empty());
}

#[tokio::test]
async fn reopening_a_node_resumes_the_active_session_without_a_second_opening() {
    let (state, _dir) = test_state().await;
    let course_id = create_route(&state, "复变函数").await;
    let nodes = course_repo::list_nodes(&state.pool, course_id)
        .await
        .expect("nodes");
    let node = nodes.first().expect("first node");

    let first = LearningService::new(&state)
        .start(node.id, None)
        .await
        .expect("first start");
    assert!(!first.resumed);

    TutorService::new(&state)
        .opening(first.session.id)
        .await
        .expect("opening");
    TutorService::new(&state)
        .turn(first.session.id, "我不太懂留数定理")
        .await
        .expect("tutor turn");
    let messages_before = loop_repo::list_messages(&state.pool, first.session.id)
        .await
        .expect("messages");
    assert_eq!(messages_before.len(), 3, "opening + user + assistant");

    let resumed = LearningService::new(&state)
        .start(node.id, None)
        .await
        .expect("resume");
    assert!(resumed.resumed);
    assert_eq!(resumed.session.id, first.session.id);
    assert_eq!(
        resumed.messages.len(),
        messages_before.len(),
        "resume must not append a second opening"
    );
}
