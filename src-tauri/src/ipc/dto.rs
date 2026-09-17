use crate::ai::schemas::{Lesson, LessonCitation};
use crate::domain::{
    Course, CourseNode, CourseNodeDependency, Job, KnowledgeObject, KnowledgeRelation,
    ProviderConfig, RouteProgress, Source,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoDto {
    pub version: String,
    pub data_dir: String,
    pub provider_count: usize,
    pub source_count: usize,
    pub course_count: usize,
    pub search_configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub search_provider: String,
    pub has_brave_api_key: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdateInput {
    pub search_provider: Option<String>,
    pub brave_api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDto {
    pub id: Uuid,
    pub display_name: String,
    pub base_url: String,
    pub chat_model: String,
    pub embedding_model: String,
    pub enabled: bool,
    pub has_api_key: bool,
}

impl ProviderDto {
    pub async fn from_config(config: ProviderConfig, has_api_key: bool) -> Self {
        Self {
            id: config.id,
            display_name: config.display_name,
            base_url: config.base_url,
            chat_model: config.chat_model,
            embedding_model: config.embedding_model,
            enabled: config.enabled,
            has_api_key,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUpsertInput {
    pub id: Option<Uuid>,
    pub display_name: String,
    pub base_url: String,
    pub chat_model: String,
    pub embedding_model: String,
    pub api_key: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTestResultDto {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDto {
    pub id: Uuid,
    pub kind: String,
    pub title: String,
    pub uri: Option<String>,
    pub original_filename: Option<String>,
    pub status: String,
    pub content_hash: Option<String>,
    pub created_at: String,
}

impl From<&Source> for SourceDto {
    fn from(source: &Source) -> Self {
        Self {
            id: source.id,
            kind: serde_json::to_value(source.kind)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| "unknown".into()),
            title: source.title.clone(),
            uri: source.uri.clone(),
            original_filename: source.original_filename.clone(),
            status: serde_json::to_value(source.status)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| "unknown".into()),
            content_hash: source.content_hash.clone(),
            created_at: source.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportBytesInput {
    pub filename: String,
    pub bytes_base64: String,
    pub title: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportUrlInput {
    pub url: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFilesInput {
    pub paths: Vec<String>,
    pub titles: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultDto {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: String,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchWebInput {
    pub query: String,
    pub count: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeSearchInput {
    pub query: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkSearchResultDto {
    pub chunk_id: Uuid,
    pub document_id: Uuid,
    pub text: String,
    pub page_number: Option<i64>,
    pub heading: Option<String>,
    pub score: f32,
    pub source_id: Option<Uuid>,
    pub source_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeObjectDto {
    pub id: Uuid,
    pub kind: String,
    pub title: String,
    pub description: Option<String>,
    pub metadata_json: String,
}

impl From<KnowledgeObject> for KnowledgeObjectDto {
    fn from(object: KnowledgeObject) -> Self {
        Self {
            id: object.id,
            kind: serde_json::to_value(object.kind)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| "unknown".into()),
            title: object.title,
            description: object.description,
            metadata_json: object.metadata_json,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeRelationDto {
    pub id: Uuid,
    pub from_object_id: Uuid,
    pub to_object_id: Uuid,
    pub relation_type: String,
    pub weight: f64,
}

impl From<KnowledgeRelation> for KnowledgeRelationDto {
    fn from(relation: KnowledgeRelation) -> Self {
        Self {
            id: relation.id,
            from_object_id: relation.from_object_id,
            to_object_id: relation.to_object_id,
            relation_type: serde_json::to_value(relation.relation_type)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| "unknown".into()),
            weight: relation.weight,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseDto {
    pub id: Uuid,
    pub topic: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub goal: Option<String>,
    pub experience_level: Option<String>,
    pub desired_depth: Option<String>,
    pub kind: String,
    pub current_node_id: Option<Uuid>,
    pub seeded: bool,
    pub node_count: i64,
    pub completed_count: i64,
    pub progress: f64,
    pub metadata_json: String,
    pub created_at: String,
}

impl CourseDto {
    pub fn from_course(course: &Course, progress: &RouteProgress) -> Self {
        Self {
            id: course.id,
            topic: course.topic.clone(),
            title: course.title.clone(),
            description: course.description.clone(),
            status: enum_name(&course.status, "unknown"),
            goal: course.goal.clone(),
            experience_level: course.experience_level.clone(),
            desired_depth: course.desired_depth.clone(),
            kind: course.kind.clone(),
            current_node_id: progress.current_node_id.or(course.current_node_id),
            seeded: course.seeded != 0,
            node_count: progress.total,
            completed_count: progress.completed,
            progress: progress.ratio(),
            metadata_json: course.metadata_json.clone(),
            created_at: course.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseNodeDto {
    pub id: Uuid,
    pub course_id: Uuid,
    pub title: String,
    pub description: String,
    pub objective: String,
    pub order_hint: i64,
    pub estimated_minutes: i64,
    pub status: String,
    pub kind: String,
    pub confidence: f64,
    pub review_at: Option<String>,
    pub generated_by: String,
    pub metadata_json: String,
}

impl From<&CourseNode> for CourseNodeDto {
    fn from(node: &CourseNode) -> Self {
        Self {
            id: node.id,
            course_id: node.course_id,
            title: node.title.clone(),
            description: node.description.clone(),
            objective: node.objective.clone(),
            order_hint: node.order_hint,
            estimated_minutes: node.estimated_minutes,
            status: enum_name(&node.status, "unknown"),
            kind: node.kind.clone(),
            confidence: node.confidence,
            review_at: node.review_at.map(|value| value.to_rfc3339()),
            generated_by: node.generated_by.clone(),
            metadata_json: node.metadata_json.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseDependencyDto {
    pub node_id: Uuid,
    pub depends_on_node_id: Uuid,
}

impl From<&CourseNodeDependency> for CourseDependencyDto {
    fn from(dependency: &CourseNodeDependency) -> Self {
        Self {
            node_id: dependency.node_id,
            depends_on_node_id: dependency.depends_on_node_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseDetailDto {
    pub course: CourseDto,
    pub nodes: Vec<CourseNodeDto>,
    pub dependencies: Vec<CourseDependencyDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseCreateInput {
    pub topic: String,
    pub goal: Option<String>,
    /// beginner | some_experience | advanced
    pub experience_level: Option<String>,
    /// overview | practical | deep
    pub desired_depth: Option<String>,
    #[serde(default)]
    pub source_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonCitationDto {
    pub chunk_id: Uuid,
    pub quote: String,
    pub text: String,
    pub page_number: Option<i64>,
    /// Name of the source the chunk came from, when it is known.
    pub source_title: Option<String>,
    /// Original link, so the learner can open the real material.
    pub uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonDto {
    pub title: String,
    pub explanation: String,
    pub key_points: Vec<String>,
    pub diagnostic_question: String,
    pub citations: Vec<LessonCitationDto>,
    pub steps: Vec<LessonStepDto>,
    pub questions: Vec<QuizQuestionDto>,
    pub mastery: f64,
    pub estimated_minutes: i64,
    pub node_kind: String,
    /// `ai`, `mock` or `deterministic`. The UI shows this so a template is
    /// never presented as a model answer.
    pub generated_by: String,
    pub has_evidence: bool,
}

impl LessonDto {
    /// Build the full learning-screen payload from a prepared node.
    pub fn from_prepared(
        prepared: &crate::services::course_service::PreparedLesson,
        node: &CourseNode,
    ) -> Self {
        let mut dto = Self::from_lesson(&prepared.lesson, &prepared.citations);
        for citation in &mut dto.citations {
            if let Some(source) = prepared
                .citation_sources
                .iter()
                .find(|source| source.chunk_id == citation.chunk_id)
            {
                citation.source_title = Some(source.source_title.clone());
                citation.uri = source.uri.clone();
            }
        }
        dto.steps = prepared.steps.iter().map(LessonStepDto::from).collect();
        dto.questions = prepared
            .questions
            .iter()
            .map(QuizQuestionDto::from)
            .collect();
        dto.mastery = prepared.mastery;
        dto.estimated_minutes = node.estimated_minutes;
        dto.node_kind = node.kind.clone();
        dto.generated_by = prepared.generated_by.clone();
        dto.has_evidence = !prepared.citations.is_empty();
        dto
    }

    pub fn from_lesson(lesson: &Lesson, chunks: &[crate::domain::Chunk]) -> Self {
        let citations = lesson
            .citations
            .iter()
            .filter_map(|citation| {
                chunks
                    .iter()
                    .find(|chunk| chunk.id == citation.chunk_id)
                    .map(|chunk| LessonCitationDto {
                        chunk_id: citation.chunk_id,
                        quote: citation.quote.clone(),
                        text: chunk.text.clone(),
                        page_number: chunk.page_number,
                        source_title: None,
                        uri: None,
                    })
            })
            .collect();
        Self {
            title: lesson.title.clone(),
            explanation: lesson.explanation.clone(),
            key_points: lesson.key_points.clone(),
            diagnostic_question: lesson.diagnostic_question.clone(),
            citations,
            steps: Vec::new(),
            questions: Vec::new(),
            mastery: 0.0,
            estimated_minutes: 0,
            node_kind: "lesson".to_owned(),
            generated_by: "deterministic".to_owned(),
            has_evidence: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningStartDto {
    pub session_id: Uuid,
    pub lesson: LessonDto,
    pub mastery: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningStartInput {
    pub node_id: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningRespondInput {
    pub session_id: Uuid,
    pub answer: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningFinishInput {
    pub session_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningEvaluationDto {
    pub feedback: String,
    pub correctness: f64,
    pub misconceptions: Vec<String>,
    pub mastery_delta: f64,
    pub follow_up_question: Option<String>,
    pub suggested_action: String,
    pub mastery: f64,
    pub attempts: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobDto {
    pub id: Uuid,
    pub kind: String,
    pub status: String,
    pub progress: f64,
    pub message: String,
    pub error_json: Option<String>,
    pub result_json: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

impl From<&Job> for JobDto {
    fn from(job: &Job) -> Self {
        Self {
            id: job.id,
            kind: job.kind.clone(),
            status: serde_json::to_value(job.status)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| "unknown".into()),
            progress: job.progress,
            message: job.message.clone(),
            error_json: job.error_json.clone(),
            result_json: job.result_json.clone(),
            created_at: job.created_at.to_rfc3339(),
            started_at: job.started_at.map(|value| value.to_rfc3339()),
            completed_at: job.completed_at.map(|value| value.to_rfc3339()),
        }
    }
}

pub fn lesson_citation_dtos(lesson: &Lesson) -> Vec<LessonCitationDto> {
    lesson
        .citations
        .iter()
        .map(|citation: &LessonCitation| LessonCitationDto {
            chunk_id: citation.chunk_id,
            quote: citation.quote.clone(),
            text: String::new(),
            page_number: None,
            source_title: None,
            uri: None,
        })
        .collect()
}

/// Serialize a `snake_case` enum through serde so DTO string values always match
/// the TypeScript union types.
pub fn enum_name<T: Serialize>(value: &T, fallback: &str) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| fallback.to_owned())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonStepDto {
    pub id: Uuid,
    pub ordinal: i64,
    pub title: String,
    pub detail: String,
    pub kind: String,
    pub duration_label: String,
}

impl From<&crate::domain::LessonStep> for LessonStepDto {
    fn from(step: &crate::domain::LessonStep) -> Self {
        Self {
            id: step.id,
            ordinal: step.ordinal,
            title: step.title.clone(),
            detail: step.detail.clone(),
            kind: step.kind.clone(),
            duration_label: step.duration_label.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuizQuestionDto {
    pub id: Uuid,
    pub node_id: Uuid,
    pub ordinal: i64,
    pub kind: String,
    pub prompt: String,
    pub options: Vec<String>,
    /// Never the raw answer key: the UI only learns it after submitting.
    pub difficulty: f64,
}

impl From<&crate::domain::QuizQuestion> for QuizQuestionDto {
    fn from(question: &crate::domain::QuizQuestion) -> Self {
        Self {
            id: question.id,
            node_id: question.node_id,
            ordinal: question.ordinal,
            kind: question.kind.clone(),
            prompt: question.prompt.clone(),
            options: question.options(),
            difficulty: question.difficulty,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuizSubmitInput {
    pub question_id: Uuid,
    pub answer: Vec<String>,
    pub session_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuizResultDto {
    pub question_id: Uuid,
    pub node_id: Uuid,
    pub correct: bool,
    /// False when the widget needs a tutor; the UI must say so instead of
    /// pretending the answer was graded.
    pub graded: bool,
    pub score: f64,
    pub feedback: String,
    pub expected: Vec<String>,
    pub misconceptions: Vec<String>,
    pub mastery: f64,
    pub confidence: f64,
    pub next_review: Option<String>,
    /// Set when the answer triggered a roadmap change.
    pub adaptation: Option<AdaptationEventDto>,
    pub inserted_node_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearnerStateDto {
    pub knowledge_point_id: Uuid,
    pub title: String,
    pub mastery: f64,
    pub confidence: f64,
    pub mistakes: i64,
    pub attempts: i64,
    pub last_seen: Option<String>,
    pub next_review: Option<String>,
    pub review_due: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdaptationEventDto {
    pub id: Uuid,
    pub course_id: Uuid,
    pub node_id: Option<Uuid>,
    pub kind: String,
    pub reason: String,
    pub payload_json: String,
    pub created_at: String,
}

impl From<&crate::domain::AdaptationEvent> for AdaptationEventDto {
    fn from(event: &crate::domain::AdaptationEvent) -> Self {
        Self {
            id: event.id,
            course_id: event.course_id,
            node_id: event.node_id,
            kind: event.kind.clone(),
            reason: event.reason.clone(),
            payload_json: event.payload_json.clone(),
            created_at: event.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorTurnInput {
    pub session_id: Uuid,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorTurnDto {
    pub message: String,
    pub suggested_action: String,
    pub diagnosis: Option<String>,
    pub generated_by: String,
    pub route_updated: bool,
    pub adaptation: Option<AdaptationEventDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseNodeCompleteDto {
    pub course: CourseDto,
    pub completed_node_id: Uuid,
    pub next_node_id: Option<Uuid>,
    pub adaptation: Option<AdaptationEventDto>,
}
