use crate::ai::prompts::{
    course_plan_messages, labeled_evidence, quiz_generation_messages, LabeledEvidence, QUIZ_V1,
};
use crate::ai::schemas::{
    parse_json, validate_course_plan, validate_quiz_generation, CoursePlan, Lesson, QuizGeneration,
};
use crate::ai::structured::generate_structured;
use crate::ai::traits::ChatRequest;
use crate::curriculum::{self, PlannedRoute, RouteBrief};
use crate::db::repo::{
    courses as course_repo, knowledge as knowledge_repo, learning as learning_repo,
    learning_loop as loop_repo,
};
use crate::domain::{
    AdaptationEvent, AdaptationKind, Course, CourseNode, CourseNodeDependency, CourseNodeStatus,
    CourseStatus, KnowledgeObject, KnowledgeObjectKind, KnowledgePoint, LessonStep, NodeKind,
    QuizQuestion, RelationType,
};
use crate::error::AppError;
use crate::jobs::manager::JobHandle;
use crate::knowledge_fs::relation::relation;
use crate::services::knowledge_service::{evidence_id_set, KnowledgeService};
use crate::state::AppState;
use crate::util::hashing::sha256_text;
use crate::util::time::utc_now;
use chrono::Utc;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Everything the learning screen needs to render one node.
#[derive(Debug, Clone)]
pub struct PreparedLesson {
    pub lesson: Lesson,
    pub citations: Vec<crate::domain::Chunk>,
    /// Where each citation came from, so the materials tab can name the source
    /// instead of showing a bare chunk.
    pub citation_sources: Vec<CitationSource>,
    pub mastery: f64,
    pub steps: Vec<LessonStep>,
    pub questions: Vec<QuizQuestion>,
    /// `ai`, `mock` or `deterministic` — surfaced in the UI so the product never
    /// pretends a template was a model.
    pub generated_by: String,
}

/// The origin of one cited chunk.
#[derive(Debug, Clone)]
pub struct CitationSource {
    pub chunk_id: Uuid,
    pub source_title: String,
    pub uri: Option<String>,
    pub kind: String,
}

/// The user's answer to "what do you want to learn?".
#[derive(Debug, Clone)]
pub struct RouteRequest {
    pub topic: String,
    pub goal: Option<String>,
    pub experience_level: Option<String>,
    pub desired_depth: Option<String>,
    pub source_ids: Vec<Uuid>,
}

impl RouteRequest {
    pub fn new(topic: impl Into<String>) -> Self {
        Self {
            topic: topic.into(),
            goal: None,
            experience_level: None,
            desired_depth: None,
            source_ids: Vec::new(),
        }
    }

    fn brief(&self) -> RouteBrief {
        RouteBrief {
            topic: self.topic.clone(),
            goal: self.goal.clone(),
            experience_level: self.experience_level.clone(),
            desired_depth: self.desired_depth.clone(),
        }
    }
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

    /// Create a roadmap.
    ///
    /// Order of preference:
    ///   1. an AI plan grounded in local evidence (when both exist),
    ///   2. the deterministic template roadmap.
    ///
    /// Step 2 is not a degraded error path: it is the shipped behaviour for
    /// every topic the learner has not imported sources for yet.
    pub async fn create_course(
        &self,
        request: RouteRequest,
        handle: Option<&JobHandle>,
    ) -> Result<Course, AppError> {
        let topic = request.topic.trim().to_owned();
        if topic.is_empty() {
            return Err(AppError::InvalidInput("course topic is required".into()));
        }
        if let Some(handle) = handle {
            handle.progress(0.05, "Collecting local evidence").await?;
        }

        let evidence = self.evidence_for_topic(&topic, &request.source_ids).await;
        let planned = if evidence.is_empty() {
            curriculum::plan_route(&request.brief())
        } else {
            match self.plan_with_ai(&topic, &evidence, handle).await {
                Ok(plan) => plan,
                Err(error) => {
                    tracing::warn!(%error, "AI roadmap planning failed; using the deterministic template");
                    curriculum::plan_route(&request.brief())
                }
            }
        };

        if let Some(handle) = handle {
            handle.progress(0.65, "Saving roadmap").await?;
        }
        let origin = planned.origin.as_str().to_owned();
        let course = self
            .save_route(&request, planned, &evidence, &origin)
            .await?;

        if let Some(handle) = handle {
            handle.progress(0.95, "Roadmap saved").await?;
        }
        Ok(course)
    }

    pub async fn regenerate_course(
        &self,
        course_id: Uuid,
        handle: Option<&JobHandle>,
    ) -> Result<Course, AppError> {
        let old_course = course_repo::get_course(&self.state.pool, course_id).await?;
        let request = RouteRequest {
            topic: old_course.topic.clone(),
            goal: old_course.goal.clone(),
            experience_level: old_course.experience_level.clone(),
            desired_depth: old_course.desired_depth.clone(),
            source_ids: source_ids_from_metadata(&old_course.metadata_json),
        };
        let new_course = self.create_course(request, handle).await?;
        course_repo::delete_course(&self.state.pool, course_id).await?;
        Ok(new_course)
    }

    async fn evidence_for_topic(
        &self,
        topic: &str,
        source_ids: &[Uuid],
    ) -> Vec<crate::knowledge::retrieval::RetrievedChunk> {
        let knowledge = KnowledgeService::new(self.state);
        if !source_ids.is_empty() {
            match knowledge.evidence_for_sources(source_ids, topic, 12).await {
                Ok(chunks) if !chunks.is_empty() => return chunks,
                Ok(_) => {}
                Err(error) => tracing::debug!(%error, "source-scoped retrieval failed"),
            }
        }
        match knowledge.semantic_search(topic, 12).await {
            Ok(chunks) => chunks,
            Err(error) => {
                tracing::debug!(%error, "semantic search unavailable; continuing without evidence");
                Vec::new()
            }
        }
    }

    async fn plan_with_ai(
        &self,
        topic: &str,
        evidence: &[crate::knowledge::retrieval::RetrievedChunk],
        handle: Option<&JobHandle>,
    ) -> Result<PlannedRoute, AppError> {
        if let Some(handle) = handle {
            handle.progress(0.30, "Requesting roadmap from AI").await?;
        }
        let known_chunk_ids: HashSet<Uuid> = evidence_id_set(evidence);
        let snippets = KnowledgeService::evidence_snippets(evidence);
        let (provider_config, chat_provider) = self.state.providers.default_chat_provider().await?;
        let messages = course_plan_messages(topic, &snippets)?;
        let known = known_chunk_ids.clone();
        let plan = generate_structured::<CoursePlan, _>(
            chat_provider.as_ref(),
            &provider_config.chat_model,
            messages,
            move |plan| validate_course_plan(plan, &known),
        )
        .await?;
        Ok(planned_route_from_ai(plan))
    }

    /// Ask the provider for quiz questions grounded in the same evidence that
    /// produced the roadmap. A failure here is not fatal: the deterministic
    /// template quiz is still available, and the caller decides whether to use
    /// it. Business validation rejects empty/unsupported questions and a single
    /// repair retry is performed by `generate_structured`.
    async fn plan_quiz_with_ai(
        &self,
        node_title: &str,
        node_objective: &str,
        concepts: &[String],
        evidence: &[crate::knowledge::retrieval::RetrievedChunk],
    ) -> Result<Vec<curriculum::PlannedQuestion>, AppError> {
        let (config, chat) = self.state.providers.default_chat_provider().await?;
        let pairs: Vec<(Uuid, Option<String>, String)> = evidence
            .iter()
            .map(|item| (item.chunk.id, None, item.chunk.text.clone()))
            .collect();
        let labeled: Vec<LabeledEvidence> = labeled_evidence(&pairs);
        let messages = quiz_generation_messages(node_title, node_objective, concepts, &labeled);
        let generation = generate_structured::<QuizGeneration, _>(
            chat.as_ref(),
            &config.chat_model,
            messages,
            validate_quiz_generation,
        )
        .await?;
        let _ = QUIZ_V1;
        Ok(generation
            .questions
            .into_iter()
            .map(|question| curriculum::PlannedQuestion {
                kind: crate::domain::QuizKind::parse(&question.kind),
                prompt: question.prompt,
                options: question.options,
                answer: question.answers,
                explanation: question.explanation,
                difficulty: question.difficulty.clamp(0.0, 1.0),
            })
            .collect())
    }

    async fn save_route(
        &self,
        request: &RouteRequest,
        planned: PlannedRoute,
        evidence: &[crate::knowledge::retrieval::RetrievedChunk],
        origin: &str,
    ) -> Result<Course, AppError> {
        let now = utc_now();
        let course_id = Uuid::new_v4();
        let topic_kind = curriculum::detect_topic_kind(&request.topic);

        let course = Course {
            id: course_id,
            topic: request.topic.trim().to_owned(),
            title: planned.title.clone(),
            description: planned.description.clone(),
            status: CourseStatus::Ready,
            metadata_json: json!({
                "learningObjectives": planned.learning_objectives,
                "prerequisites": planned.prerequisites,
                "sourceIds": request.source_ids,
                "evidenceChunkIds": evidence.iter().map(|chunk| chunk.chunk.id).collect::<Vec<_>>(),
                "origin": origin,
            })
            .to_string(),
            goal: request.goal.clone(),
            experience_level: request.experience_level.clone(),
            desired_depth: request.desired_depth.clone(),
            kind: topic_kind.as_str().to_owned(),
            current_node_id: None,
            seeded: 0,
            created_at: now,
            updated_at: now,
        };

        // Node ids are deterministic per key so dependencies can be resolved
        // before anything is written.
        let mut node_ids: HashMap<String, Uuid> = HashMap::new();
        for planned_node in &planned.nodes {
            node_ids.insert(planned_node.key.clone(), Uuid::new_v4());
        }

        let evidence_ids: Vec<Uuid> = evidence.iter().map(|chunk| chunk.chunk.id).collect();

        let mut nodes: Vec<CourseNode> = Vec::with_capacity(planned.nodes.len());
        let mut knowledge_points: Vec<KnowledgePoint> = Vec::with_capacity(planned.nodes.len());
        let mut node_points: Vec<(Uuid, Vec<Uuid>)> = Vec::new();
        let mut lesson_steps: Vec<LessonStep> = Vec::new();
        let mut questions: Vec<QuizQuestion> = Vec::new();

        for (index, planned_node) in planned.nodes.iter().enumerate() {
            let node_id = node_ids[&planned_node.key];
            let evidence_for_node: Vec<Uuid> = if evidence_ids.is_empty() {
                Vec::new()
            } else {
                vec![evidence_ids[index % evidence_ids.len()]]
            };
            let point_id = Uuid::new_v4();
            let point = KnowledgePoint {
                id: point_id,
                course_id,
                title: planned_node.title.clone(),
                description: planned_node.objective.clone(),
                ordinal: index as i64,
                created_at: now,
            };

            let planned_lesson = curriculum::plan_lesson(
                &planned_node.title,
                &planned_node.objective,
                &planned_node.concepts,
                &course.title,
            );
            let lesson = Lesson {
                title: planned_lesson.title.clone(),
                explanation: planned_lesson.explanation.clone(),
                key_points: planned_lesson.key_points.clone(),
                diagnostic_question: planned_lesson.diagnostic_question.clone(),
                citations: Vec::new(),
            };

            let node = CourseNode {
                id: node_id,
                course_id,
                title: planned_node.title.clone(),
                description: planned_node.description.clone(),
                objective: planned_node.objective.clone(),
                order_hint: index as i64,
                estimated_minutes: planned_node.estimated_minutes,
                status: if index == 0 {
                    CourseNodeStatus::Available
                } else {
                    CourseNodeStatus::Locked
                },
                metadata_json: json!({
                    "concepts": planned_node.concepts,
                    "dependencies": planned_node.dependencies,
                    "evidenceChunkIds": evidence_for_node,
                    "knowledgePointId": point_id,
                    "origin": origin,
                })
                .to_string(),
                lesson_cache_json: Some(
                    serde_json::to_string(&lesson)
                        .map_err(|error| AppError::Internal(error.to_string()))?,
                ),
                lesson_cache_hash: Some(sha256_text(&format!("{origin}:{}", planned_node.key))),
                kind: planned_node.kind.as_str().to_owned(),
                confidence: 0.0,
                review_at: None,
                generated_by: origin.to_owned(),
                created_at: now,
                updated_at: now,
            };

            for (ordinal, (title, detail, kind, duration)) in curriculum::plan_steps(
                &planned_node.title,
                &planned_node.concepts,
                planned_node.estimated_minutes,
            )
            .into_iter()
            .enumerate()
            {
                lesson_steps.push(LessonStep::new(
                    node_id,
                    ordinal as i64,
                    title,
                    detail,
                    kind,
                    duration,
                ));
            }

            let ai_quiz = if origin == "ai" {
                match self
                    .plan_quiz_with_ai(
                        &planned_node.title,
                        &planned_node.objective,
                        &planned_node.concepts,
                        evidence,
                    )
                    .await
                {
                    Ok(questions) if !questions.is_empty() => Some(questions),
                    Ok(_) => None,
                    Err(error) => {
                        tracing::warn!(%error, node = %planned_node.title, "AI quiz generation failed; using the deterministic quiz");
                        None
                    }
                }
            } else {
                None
            };
            let planned_questions = ai_quiz.unwrap_or_else(|| {
                curriculum::plan_quiz(
                    &planned_node.title,
                    &planned_node.objective,
                    &planned_node.concepts,
                )
            });
            for (ordinal, planned_question) in planned_questions.into_iter().enumerate() {
                questions.push(QuizQuestion {
                    id: Uuid::new_v4(),
                    node_id,
                    knowledge_point_id: Some(point_id),
                    ordinal: ordinal as i64,
                    kind: planned_question.kind.as_str().to_owned(),
                    prompt: planned_question.prompt,
                    options_json: serde_json::to_string(&planned_question.options)
                        .map_err(|error| AppError::Internal(error.to_string()))?,
                    answer_json: serde_json::to_string(&planned_question.answer)
                        .map_err(|error| AppError::Internal(error.to_string()))?,
                    explanation: planned_question.explanation,
                    difficulty: planned_question.difficulty,
                    created_at: now,
                });
            }

            node_points.push((node_id, vec![point_id]));
            knowledge_points.push(point);
            nodes.push(node);
        }

        let dependencies: Vec<CourseNodeDependency> = planned
            .nodes
            .iter()
            .flat_map(|planned_node| {
                let node_id = node_ids.get(&planned_node.key).copied();
                let map = &node_ids;
                planned_node
                    .dependencies
                    .iter()
                    .filter_map(move |dependency| {
                        let depends_on = map.get(dependency).copied();
                        match (node_id, depends_on) {
                            (Some(node_id), Some(depends_on_node_id)) => {
                                Some(CourseNodeDependency {
                                    course_id,
                                    node_id,
                                    depends_on_node_id,
                                })
                            }
                            _ => None,
                        }
                    })
            })
            .collect();

        course_repo::insert_course_graph(&self.state.pool, &course, &nodes, &dependencies).await?;
        loop_repo::insert_knowledge_points(&self.state.pool, &knowledge_points).await?;
        for (node_id, point_ids) in &node_points {
            loop_repo::link_node_points(&self.state.pool, *node_id, point_ids).await?;
        }
        for node in &nodes {
            loop_repo::replace_lesson_steps(
                &self.state.pool,
                node.id,
                &lesson_steps
                    .iter()
                    .filter(|step| step.node_id == node.id)
                    .cloned()
                    .collect::<Vec<_>>(),
            )
            .await?;
            loop_repo::replace_quiz_questions(
                &self.state.pool,
                node.id,
                &questions
                    .iter()
                    .filter(|question| question.node_id == node.id)
                    .cloned()
                    .collect::<Vec<_>>(),
            )
            .await?;
        }

        // The first node becomes "从这里继续".
        if let Some(first) = nodes.first() {
            course_repo::set_current_node(&self.state.pool, course_id, Some(first.id)).await?;
        }

        self.index_route_knowledge(&course, &nodes).await?;

        loop_repo::insert_adaptation(
            &self.state.pool,
            &AdaptationEvent::new(
                course_id,
                None,
                AdaptationKind::LessonReplanned,
                format!("路线由 {origin} 生成"),
                json!({ "nodes": nodes.len(), "origin": origin }),
            ),
        )
        .await?;

        course_repo::get_course(&self.state.pool, course_id).await
    }

    /// Mirror the roadmap into the generic knowledge graph so search, notes and
    /// the `knowledge_*` commands can see it.
    async fn index_route_knowledge(
        &self,
        course: &Course,
        nodes: &[CourseNode],
    ) -> Result<(), AppError> {
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

        for node in nodes {
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

        let dependencies = course_repo::list_dependencies(&self.state.pool, course.id).await?;
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
        Ok(())
    }

    /// Build the lesson for one node.
    pub async fn prepare_node(
        &self,
        node_id: Uuid,
        handle: Option<&JobHandle>,
    ) -> Result<PreparedLesson, AppError> {
        let node = course_repo::get_node(&self.state.pool, node_id).await?;
        let knowledge = KnowledgeService::new(self.state);
        let evidence = match knowledge.evidence_for_node(&node, 12).await {
            Ok(chunks) => chunks,
            Err(error) => {
                tracing::debug!(%error, "node evidence unavailable");
                Vec::new()
            }
        };
        let citations: Vec<crate::domain::Chunk> =
            evidence.iter().map(|chunk| chunk.chunk.clone()).collect();
        let mastery = learning_repo::get_mastery(&self.state.pool, node.id)
            .await?
            .mastery;
        let steps = loop_repo::list_lesson_steps(&self.state.pool, node.id).await?;
        let questions = loop_repo::list_quiz_questions(&self.state.pool, node.id).await?;

        let cached = node
            .lesson_cache_json
            .as_deref()
            .and_then(|value| parse_json::<Lesson>(value).ok());

        // Try the model only when there is something to ground it in.
        if !evidence.is_empty() {
            match self.ai_lesson(&node, &evidence, mastery, handle).await {
                Ok(lesson) => {
                    return Ok(PreparedLesson {
                        lesson,
                        citations: citations.clone(),
                        citation_sources: self.citation_sources(&citations).await,
                        mastery,
                        steps,
                        questions,
                        generated_by: "ai".to_owned(),
                    });
                }
                Err(error) => {
                    tracing::warn!(%error, node = %node.id, "AI lesson generation failed; using the template lesson");
                }
            }
        }

        let lesson = match cached {
            Some(lesson) => lesson,
            None => self.template_lesson(&node)?,
        };
        Ok(PreparedLesson {
            lesson,
            citations: citations.clone(),
            citation_sources: self.citation_sources(&citations).await,
            mastery,
            steps,
            questions,
            generated_by: node.generated_by.clone(),
        })
    }

    /// One join instead of a query per chunk: citations are shown as a list.
    async fn citation_sources(&self, chunks: &[crate::domain::Chunk]) -> Vec<CitationSource> {
        if chunks.is_empty() {
            return Vec::new();
        }
        let placeholders = vec!["?"; chunks.len()].join(",");
        let sql = format!(
            "SELECT c.id, s.title, s.uri, s.kind FROM chunks c \
             JOIN documents d ON d.id = c.document_id \
             JOIN sources s ON s.id = d.source_id WHERE c.id IN ({placeholders})"
        );
        let mut statement = sqlx::query_as::<_, (Uuid, String, Option<String>, String)>(&sql);
        for chunk in chunks {
            statement = statement.bind(chunk.id);
        }
        match statement.fetch_all(&self.state.pool).await {
            Ok(rows) => rows
                .into_iter()
                .map(|(chunk_id, source_title, uri, kind)| CitationSource {
                    chunk_id,
                    source_title,
                    uri,
                    kind,
                })
                .collect(),
            Err(error) => {
                tracing::debug!(%error, "citation sources unavailable");
                Vec::new()
            }
        }
    }

    async fn ai_lesson(
        &self,
        node: &CourseNode,
        evidence: &[crate::knowledge::retrieval::RetrievedChunk],
        mastery: f64,
        handle: Option<&JobHandle>,
    ) -> Result<Lesson, AppError> {
        let source_hash = source_hash(
            &evidence
                .iter()
                .map(|chunk| chunk.chunk.clone())
                .collect::<Vec<_>>(),
        );
        if node.lesson_cache_hash.as_deref() == Some(source_hash.as_str()) {
            if let Some(cached) = node
                .lesson_cache_json
                .as_deref()
                .and_then(|value| parse_json::<Lesson>(value).ok())
            {
                if !cached.citations.is_empty() {
                    return Ok(cached);
                }
            }
        }
        if let Some(handle) = handle {
            handle.progress(0.35, "Generating lesson").await?;
        }
        let misconceptions = previous_misconceptions_for_node(&self.state.pool, node.id).await?;
        let snippets = KnowledgeService::evidence_snippets(evidence);
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
        crate::ai::schemas::validate_lesson_citations(&lesson, &known)?;
        let lesson_json = serde_json::to_string(&lesson)
            .map_err(|error| AppError::Internal(error.to_string()))?;
        course_repo::update_node_lesson_cache(
            &self.state.pool,
            node.id,
            &lesson_json,
            &source_hash,
        )
        .await?;
        Ok(lesson)
    }

    fn template_lesson(&self, node: &CourseNode) -> Result<Lesson, AppError> {
        let concepts: Vec<String> = serde_json::from_str::<serde_json::Value>(&node.metadata_json)
            .ok()
            .and_then(|value| value.get("concepts").cloned())
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default();
        let planned = curriculum::plan_lesson(&node.title, &node.objective, &concepts, &node.title);
        Ok(Lesson {
            title: planned.title,
            explanation: planned.explanation,
            key_points: planned.key_points,
            diagnostic_question: planned.diagnostic_question,
            citations: Vec::new(),
        })
    }

    /// Mark a node complete, unlock the next one and move "从这里继续".
    pub async fn complete_node(&self, node_id: Uuid) -> Result<Course, AppError> {
        let node = course_repo::get_node(&self.state.pool, node_id).await?;
        course_repo::update_node_status(&self.state.pool, node_id, CourseNodeStatus::Completed)
            .await?;

        let nodes = course_repo::list_nodes(&self.state.pool, node.course_id).await?;
        let next = nodes
            .iter()
            .find(|candidate| {
                candidate.order_hint > node.order_hint
                    && candidate.status != CourseNodeStatus::Completed
            })
            .cloned();
        if let Some(next) = &next {
            if next.status == CourseNodeStatus::Locked {
                course_repo::update_node_status(
                    &self.state.pool,
                    next.id,
                    CourseNodeStatus::Available,
                )
                .await?;
            }
        }
        course_repo::set_current_node(
            &self.state.pool,
            node.course_id,
            next.as_ref().map(|node| node.id),
        )
        .await?;
        loop_repo::insert_adaptation(
            &self.state.pool,
            &AdaptationEvent::new(
                node.course_id,
                Some(node.id),
                AdaptationKind::NodeUnlocked,
                format!("完成「{}」", node.title),
                json!({ "nextNodeId": next.as_ref().map(|node| node.id) }),
            ),
        )
        .await?;
        course_repo::get_course(&self.state.pool, node.course_id).await
    }

    /// Insert a targeted practice node directly after `after_node_id`.
    ///
    /// This is the real version of the prototype's `M().lessons[2] = '加练 · …'`.
    pub async fn insert_review_node(
        &self,
        course_id: Uuid,
        after_node_id: Uuid,
        title: &str,
        objective: &str,
        reason: &str,
        point_id: Option<Uuid>,
    ) -> Result<CourseNode, AppError> {
        let course = course_repo::get_course(&self.state.pool, course_id).await?;
        let after = course_repo::get_node(&self.state.pool, after_node_id).await?;
        let now = utc_now();

        course_repo::shift_order_hints(&self.state.pool, course_id, after.order_hint + 1).await?;

        let node_id = Uuid::new_v4();
        let point_id = point_id.unwrap_or_else(Uuid::new_v4);
        let node = CourseNode {
            id: node_id,
            course_id,
            title: title.to_owned(),
            description: objective.to_owned(),
            objective: objective.to_owned(),
            order_hint: after.order_hint + 1,
            estimated_minutes: 8,
            status: CourseNodeStatus::Available,
            metadata_json: json!({
                "concepts": [title],
                "dependencies": [after_node_id],
                "knowledgePointId": point_id,
                "insertedFor": after_node_id,
                "reason": reason,
            })
            .to_string(),
            lesson_cache_json: None,
            lesson_cache_hash: None,
            kind: NodeKind::Review.as_str().to_owned(),
            confidence: 0.0,
            review_at: Some(Utc::now()),
            generated_by: "adaptation".to_owned(),
            created_at: now,
            updated_at: now,
        };

        course_repo::insert_node_row(&self.state.pool, &node).await?;

        // The review node revises the same knowledge point as the node it follows.
        let point = loop_repo::points_for_node(&self.state.pool, after_node_id).await?;
        if let Some(point) = point.first() {
            loop_repo::link_node_points(&self.state.pool, node_id, &[point.id]).await?;
        }

        let steps =
            curriculum::plan_steps(&node.title, &[title.to_owned()], node.estimated_minutes);
        let lesson_steps: Vec<LessonStep> = steps
            .into_iter()
            .enumerate()
            .map(|(ordinal, (step_title, detail, kind, duration))| {
                LessonStep::new(node_id, ordinal as i64, step_title, detail, kind, duration)
            })
            .collect();
        loop_repo::replace_lesson_steps(&self.state.pool, node_id, &lesson_steps).await?;

        let questions: Vec<QuizQuestion> =
            curriculum::plan_quiz(&node.title, &node.objective, &[title.to_owned()])
                .into_iter()
                .enumerate()
                .map(|(ordinal, planned)| QuizQuestion {
                    id: Uuid::new_v4(),
                    node_id,
                    knowledge_point_id: None,
                    ordinal: ordinal as i64,
                    kind: planned.kind.as_str().to_owned(),
                    prompt: planned.prompt,
                    options_json: serde_json::to_string(&planned.options)
                        .unwrap_or_else(|_| "[]".into()),
                    answer_json: serde_json::to_string(&planned.answer)
                        .unwrap_or_else(|_| "[]".into()),
                    explanation: planned.explanation,
                    difficulty: planned.difficulty,
                    created_at: now,
                })
                .collect();
        loop_repo::replace_quiz_questions(&self.state.pool, node_id, &questions).await?;

        let node_object = KnowledgeObject {
            id: node_id,
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
            &relation(course.id, node_id, RelationType::Contains, 1.0)?,
        )
        .await?;

        loop_repo::insert_adaptation(
            &self.state.pool,
            &AdaptationEvent::new(
                course_id,
                Some(after_node_id),
                AdaptationKind::ReviewInserted,
                reason.to_owned(),
                json!({ "reviewNodeId": node_id, "title": node.title }),
            ),
        )
        .await?;

        Ok(node)
    }
}

fn source_ids_from_metadata(metadata_json: &str) -> Vec<Uuid> {
    serde_json::from_str::<serde_json::Value>(metadata_json)
        .ok()
        .and_then(|value| value.get("sourceIds").cloned())
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

/// Convert an AI-produced plan into the same shape as the deterministic one.
fn planned_route_from_ai(plan: CoursePlan) -> PlannedRoute {
    PlannedRoute {
        title: plan.title,
        description: plan.description,
        learning_objectives: plan.learning_objectives,
        prerequisites: plan.prerequisites,
        nodes: plan
            .nodes
            .into_iter()
            .map(|node| curriculum::PlannedNode {
                key: node.id,
                title: node.title,
                objective: node.objective,
                description: node.description,
                concepts: node.concepts,
                dependencies: node.dependencies,
                kind: NodeKind::Lesson,
                estimated_minutes: node.estimated_minutes,
            })
            .collect(),
        origin: curriculum::PlanOrigin::Ai,
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
