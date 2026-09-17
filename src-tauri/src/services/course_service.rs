use crate::ai::prompts::course_plan_messages;
use crate::ai::schemas::{
    parse_json, validate_course_plan, validate_lesson_citations, CoursePlan, Lesson,
};
use crate::ai::traits::ChatRequest;
use crate::db::repo::{
    courses as course_repo, knowledge as knowledge_repo, learning as learning_repo,
};
use crate::domain::{
    Course, CourseNode, CourseNodeDependency, CourseNodeStatus, CourseStatus, KnowledgeObject,
    KnowledgeObjectKind, RelationType,
};
use crate::error::AppError;
use crate::jobs::manager::JobHandle;
use crate::knowledge_fs::relation::relation;
use crate::services::knowledge_service::{evidence_id_set, KnowledgeService};
use crate::state::AppState;
use crate::util::hashing::sha256_text;
use crate::util::time::utc_now;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PreparedLesson {
    pub lesson: Lesson,
    pub citations: Vec<crate::domain::Chunk>,
    pub mastery: f64,
}

pub struct CourseService<'a> {
    state: &'a AppState,
}

impl<'a> CourseService<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub async fn list_courses(&self) -> Result<Vec<Course>, AppError> {
        course_repo::list_courses(&self.state.pool).await
    }

    pub async fn get_course_bundle(
        &self,
        id: Uuid,
    ) -> Result<(Course, Vec<CourseNode>, Vec<CourseNodeDependency>), AppError> {
        let course = course_repo::get_course(&self.state.pool, id).await?;
        let nodes = course_repo::list_nodes(&self.state.pool, id).await?;
        let dependencies = course_repo::list_dependencies(&self.state.pool, id).await?;
        Ok((course, nodes, dependencies))
    }

    pub async fn delete_course(&self, id: Uuid) -> Result<(), AppError> {
        course_repo::delete_course(&self.state.pool, id).await
    }

    pub async fn create_course(
        &self,
        topic: &str,
        handle: Option<&JobHandle>,
    ) -> Result<Course, AppError> {
        let topic = topic.trim();
        if topic.is_empty() {
            return Err(AppError::InvalidInput("course topic is required".into()));
        }
        if let Some(handle) = handle {
            handle.progress(0.05, "Retrieving local evidence").await?;
        }
        let knowledge = KnowledgeService::new(self.state);
        let evidence = knowledge.semantic_search(topic, 12).await?;
        if evidence.is_empty() {
            return Err(AppError::InvalidInput(
                "no local evidence found; import source files or a URL first".into(),
            ));
        }
        let known_chunk_ids: HashSet<Uuid> = evidence_id_set(&evidence);
        let snippets = KnowledgeService::evidence_snippets(&evidence);

        if let Some(handle) = handle {
            handle
                .progress(0.30, "Requesting course plan from AI")
                .await?;
        }
        let (provider_config, chat_provider) = self.state.providers.default_chat_provider().await?;
        let messages = course_plan_messages(topic, &snippets)?;
        let response = chat_provider
            .chat(ChatRequest::json(
                provider_config.chat_model.clone(),
                messages,
            ))
            .await?;
        let plan: CoursePlan = parse_json(&response.content)?;
        validate_course_plan(&plan, &known_chunk_ids)?;

        if let Some(handle) = handle {
            handle.progress(0.70, "Saving roadmap").await?;
        }
        let course = self.save_course_plan(topic, plan).await?;
        if let Some(handle) = handle {
            handle.progress(0.95, "Course roadmap saved").await?;
        }
        Ok(course)
    }

    pub async fn regenerate_course(
        &self,
        course_id: Uuid,
        handle: Option<&JobHandle>,
    ) -> Result<Course, AppError> {
        let old_course = course_repo::get_course(&self.state.pool, course_id).await?;
        let new_course = self.create_course(&old_course.topic, handle).await?;
        course_repo::delete_course(&self.state.pool, course_id).await?;
        Ok(new_course)
    }

    pub async fn prepare_node(
        &self,
        node_id: Uuid,
        handle: Option<&JobHandle>,
    ) -> Result<PreparedLesson, AppError> {
        let node = course_repo::get_node(&self.state.pool, node_id).await?;
        let knowledge = KnowledgeService::new(self.state);
        let evidence = knowledge.evidence_for_node(&node, 12).await?;
        if evidence.is_empty() {
            return Err(AppError::InvalidInput(
                "this node has no evidence chunks available".into(),
            ));
        }
        let citations: Vec<crate::domain::Chunk> =
            evidence.iter().map(|chunk| chunk.chunk.clone()).collect();
        let source_hash = source_hash(&citations);

        if let Some(cached) = node.lesson_cache_json.as_deref() {
            if node.lesson_cache_hash.as_deref() == Some(source_hash.as_str()) {
                if let Ok(lesson) = parse_json::<Lesson>(cached) {
                    let mastery = learning_repo::get_mastery(&self.state.pool, node.id)
                        .await?
                        .mastery;
                    return Ok(PreparedLesson {
                        lesson,
                        citations,
                        mastery,
                    });
                }
            }
        }

        if let Some(handle) = handle {
            handle.progress(0.35, "Generating lesson").await?;
        }
        let mastery = learning_repo::get_mastery(&self.state.pool, node.id)
            .await?
            .mastery;
        let misconceptions = previous_misconceptions_for_node(&self.state.pool, node.id).await?;
        let snippets = KnowledgeService::evidence_snippets(&evidence);
        let (provider_config, chat_provider) = self.state.providers.default_chat_provider().await?;
        let messages = crate::ai::prompts::lesson_messages(
            &node.title,
            &node.objective,
            mastery,
            &misconceptions,
            &snippets,
        )?;
        let response = chat_provider
            .chat(ChatRequest::json(
                provider_config.chat_model.clone(),
                messages,
            ))
            .await?;
        let lesson: Lesson = parse_json(&response.content)?;
        let known: HashSet<Uuid> = evidence.iter().map(|chunk| chunk.chunk.id).collect();
        validate_lesson_citations(&lesson, &known)?;

        let lesson_json = serde_json::to_string(&lesson)
            .map_err(|error| AppError::Internal(error.to_string()))?;
        course_repo::update_node_lesson_cache(
            &self.state.pool,
            node.id,
            &lesson_json,
            &source_hash,
        )
        .await?;

        if let Some(handle) = handle {
            handle.progress(0.90, "Lesson ready").await?;
        }
        Ok(PreparedLesson {
            lesson,
            citations,
            mastery,
        })
    }

    async fn save_course_plan(&self, topic: &str, plan: CoursePlan) -> Result<Course, AppError> {
        let now = utc_now();
        let course_id = Uuid::new_v4();
        let course = Course {
            id: course_id,
            topic: topic.to_owned(),
            title: plan.title.clone(),
            description: plan.description.clone(),
            status: CourseStatus::Ready,
            metadata_json: json!({
                "learningObjectives": plan.learning_objectives,
                "prerequisites": plan.prerequisites,
            })
            .to_string(),
            created_at: now,
            updated_at: now,
        };

        let mut node_ids: HashMap<String, Uuid> = HashMap::new();
        for plan_node in &plan.nodes {
            node_ids.insert(plan_node.id.clone(), Uuid::new_v4());
        }

        let nodes: Vec<CourseNode> = plan
            .nodes
            .iter()
            .enumerate()
            .map(|(index, plan_node)| {
                let id = node_ids
                    .get(&plan_node.id)
                    .copied()
                    .unwrap_or_else(Uuid::new_v4);
                CourseNode {
                    id,
                    course_id,
                    title: plan_node.title.clone(),
                    description: plan_node.description.clone(),
                    objective: plan_node.objective.clone(),
                    order_hint: index as i64,
                    estimated_minutes: plan_node.estimated_minutes,
                    status: if plan_node.dependencies.is_empty() {
                        CourseNodeStatus::Available
                    } else {
                        CourseNodeStatus::Locked
                    },
                    metadata_json: json!({
                        "concepts": plan_node.concepts,
                        "dependencies": plan_node.dependencies,
                        "evidenceChunkIds": plan_node.evidence_chunk_ids,
                        "clientId": plan_node.id,
                    })
                    .to_string(),
                    lesson_cache_json: None,
                    lesson_cache_hash: None,
                    created_at: now,
                    updated_at: now,
                }
            })
            .collect();

        let dependencies: Vec<CourseNodeDependency> = plan
            .nodes
            .iter()
            .flat_map(|plan_node| {
                let node_id = node_ids.get(&plan_node.id).copied();
                let map = &node_ids;
                plan_node.dependencies.iter().filter_map(move |dependency| {
                    let depends_on = map.get(dependency).copied();
                    match (node_id, depends_on) {
                        (Some(node_id), Some(depends_on_node_id)) => Some(CourseNodeDependency {
                            course_id,
                            node_id,
                            depends_on_node_id,
                        }),
                        _ => None,
                    }
                })
            })
            .collect();

        course_repo::insert_course_graph(&self.state.pool, &course, &nodes, &dependencies).await?;

        let course_object = KnowledgeObject {
            id: course.id,
            kind: KnowledgeObjectKind::Course,
            title: course.title.clone(),
            description: Some(course.description.clone()),
            metadata_json: course.metadata_json.clone(),
            created_at: course.created_at,
            updated_at: course.updated_at,
        };
        knowledge_repo::upsert_object(&self.state.pool, &course_object).await?;

        for node in &nodes {
            let node_object = KnowledgeObject {
                id: node.id,
                kind: KnowledgeObjectKind::CourseNode,
                title: node.title.clone(),
                description: Some(node.objective.clone()),
                metadata_json: node.metadata_json.clone(),
                created_at: node.created_at,
                updated_at: node.updated_at,
            };
            knowledge_repo::upsert_object(&self.state.pool, &node_object).await?;
            knowledge_repo::insert_relation(
                &self.state.pool,
                &relation(course.id, node.id, RelationType::Contains, 1.0)?,
            )
            .await?;
            knowledge_repo::insert_relation(
                &self.state.pool,
                &relation(node.id, course.id, RelationType::PartOf, 1.0)?,
            )
            .await?;
        }

        for dependency in &dependencies {
            knowledge_repo::insert_relation(
                &self.state.pool,
                &relation(
                    dependency.depends_on_node_id,
                    dependency.node_id,
                    RelationType::PrerequisiteOf,
                    1.0,
                )?,
            )
            .await?;
        }

        Ok(course)
    }
}

fn source_hash(citations: &[crate::domain::Chunk]) -> String {
    let combined = citations
        .iter()
        .map(|chunk| chunk.content_hash.as_str())
        .collect::<Vec<_>>()
        .join("|");
    sha256_text(&combined)
}

pub async fn previous_misconceptions_for_node(
    pool: &sqlx::SqlitePool,
    node_id: Uuid,
) -> Result<Vec<String>, AppError> {
    let attempts = learning_repo::list_attempts_for_node(pool, node_id).await?;
    let mut output = Vec::new();
    for attempt in attempts {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&attempt.evaluation_json) else {
            continue;
        };
        if let Some(values) = value
            .get("misconceptions")
            .and_then(|value| value.as_array())
        {
            for value in values {
                if let Some(text) = value.as_str() {
                    if !text.trim().is_empty() && !output.contains(&text.to_owned()) {
                        output.push(text.to_owned());
                    }
                }
            }
        }
    }
    output.truncate(10);
    Ok(output)
}
