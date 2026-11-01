use super::dto::{
    ChunkSearchResultDto, KnowledgeObjectDto, KnowledgeRelationDto, KnowledgeSearchInput,
    RetrievalDebugInput, RetrievalHitDto,
};
use crate::domain::RelationType;
use crate::error::AppError;
use crate::ipc::State;
use crate::knowledge::hybrid::{HybridRetriever, RetrievalFilter};
use crate::knowledge_fs::query as knowledge_query;
use crate::services::knowledge_service::KnowledgeService;
use crate::state::AppState;
use uuid::Uuid;

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn knowledge_search(
    state: State<'_, AppState>,
    input: KnowledgeSearchInput,
) -> Result<Vec<ChunkSearchResultDto>, AppError> {
    let results = KnowledgeService::new(&state)
        .semantic_search(&input.query, input.limit.unwrap_or(10).clamp(1, 50))
        .await?;
    let knowledge = KnowledgeService::new(&state);
    let mut output = Vec::with_capacity(results.len());
    for result in results {
        let source = knowledge.source_for_chunk(&result.chunk).await.ok();
        output.push(ChunkSearchResultDto {
            chunk_id: result.chunk.id,
            document_id: result.chunk.document_id,
            text: result.chunk.text,
            page_number: result.chunk.page_number,
            heading: result.chunk.heading,
            score: result.score,
            source_id: source.as_ref().map(|value| value.id),
            source_title: source.map(|value| value.title),
        });
    }
    Ok(output)
}

/// Development/debug inspector: returns the exact hybrid hit list, including
/// the lexical and vector ranks, without pretending it is a production search.
#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn knowledge_retrieval_debug(
    state: State<'_, AppState>,
    input: RetrievalDebugInput,
) -> Result<Vec<RetrievalHitDto>, AppError> {
    let filter = RetrievalFilter {
        route_id: input.route_id,
        source_ids: input.source_ids,
        knowledge_point_ids: input.knowledge_point_ids,
        top_k: input.top_k.unwrap_or(8).clamp(1, 20),
    };
    let hits = HybridRetriever::new(&state)
        .retrieve(&input.query, filter)
        .await?;
    Ok(hits
        .into_iter()
        .map(|hit| RetrievalHitDto {
            chunk_id: hit.chunk_id,
            source_id: hit.source_id,
            source_title: hit.source_title,
            text: hit.text,
            vector_rank: hit.vector_rank,
            lexical_rank: hit.lexical_rank,
            vector_score: hit.vector_score,
            lexical_score: hit.lexical_score,
            combined_score: hit.combined_score,
        })
        .collect())
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn knowledge_get_object(
    state: State<'_, AppState>,
    id: Option<Uuid>,
    path: Option<String>,
) -> Result<KnowledgeObjectDto, AppError> {
    let object = match (id, path) {
        (_, Some(path)) => knowledge_query::resolve_virtual_path(&state.pool, &path).await?,
        (Some(id), None) => knowledge_query::get_object(&state.pool, id).await?,
        (None, None) => {
            return Err(AppError::InvalidInput(
                "either id or path is required".into(),
            ))
        }
    };
    Ok(object.into())
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn knowledge_get_related(
    state: State<'_, AppState>,
    object_id: Uuid,
    relation_type: Option<String>,
) -> Result<Vec<KnowledgeRelationDto>, AppError> {
    let relation_type = match relation_type.as_deref() {
        Some(value) if !value.trim().is_empty() => Some(parse_relation_type(value)?),
        _ => None,
    };
    let relations = knowledge_query::get_related(&state.pool, object_id, relation_type).await?;
    Ok(relations
        .into_iter()
        .map(KnowledgeRelationDto::from)
        .collect())
}

fn parse_relation_type(value: &str) -> Result<RelationType, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    let relation = match normalized.as_str() {
        "contains" => RelationType::Contains,
        "derived_from" | "derivedfrom" => RelationType::DerivedFrom,
        "mentions" => RelationType::Mentions,
        "discusses" => RelationType::Discusses,
        "supports" => RelationType::Supports,
        "related_to" | "relatedto" => RelationType::RelatedTo,
        "prerequisite_of" | "prerequisiteof" => RelationType::PrerequisiteOf,
        "part_of" | "partof" => RelationType::PartOf,
        "appears_in" | "appearsin" => RelationType::AppearsIn,
        _ => {
            return Err(AppError::InvalidInput(format!(
                "unknown relation type: {value}"
            )))
        }
    };
    Ok(relation)
}
