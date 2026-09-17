use super::virtual_path::VirtualPath;
use crate::db::repo::{
    chunks as chunk_repo, courses as course_repo, knowledge as knowledge_repo,
    sources as source_repo,
};
use crate::domain::{
    KnowledgeObject, KnowledgeObjectKind, KnowledgeRelation, RelationType, Source, SourceKind,
};
use crate::error::AppError;
use sqlx::SqlitePool;
use uuid::Uuid;

pub async fn get_object(pool: &SqlitePool, id: Uuid) -> Result<KnowledgeObject, AppError> {
    knowledge_repo::get_object(pool, id).await
}

pub async fn get_related(
    pool: &SqlitePool,
    object_id: Uuid,
    relation_type: Option<RelationType>,
) -> Result<Vec<KnowledgeRelation>, AppError> {
    knowledge_repo::get_related(pool, object_id, relation_type).await
}

pub async fn query_relations(
    pool: &SqlitePool,
    from_object_id: Option<Uuid>,
    to_object_id: Option<Uuid>,
    relation_type: Option<RelationType>,
) -> Result<Vec<KnowledgeRelation>, AppError> {
    knowledge_repo::query_relations(pool, from_object_id, to_object_id, relation_type).await
}

pub async fn resolve_virtual_path(
    pool: &SqlitePool,
    path: &str,
) -> Result<KnowledgeObject, AppError> {
    let virtual_path = VirtualPath::parse(path)?;
    match virtual_path {
        VirtualPath::Source(id) => resolve_source(pool, id).await,
        VirtualPath::SourceDocument(source_id, document_id) => {
            let document = source_repo::get_document(pool, document_id).await?;
            if document.source_id != source_id {
                return Err(AppError::NotFound(
                    "document does not belong to the source in this path".into(),
                ));
            }
            Ok(KnowledgeObject {
                id: document.id,
                kind: KnowledgeObjectKind::Document,
                title: document.title,
                description: Some(document.text_content.chars().take(240).collect()),
                metadata_json: document.metadata_json,
                created_at: document.created_at,
                updated_at: document.created_at,
            })
        }
        VirtualPath::Concept(id) => knowledge_repo::get_object(pool, id).await,
        VirtualPath::Course(id) => resolve_course(pool, id).await,
        VirtualPath::CourseNode(course_id, node_id) => {
            let node = course_repo::get_node(pool, node_id).await?;
            if node.course_id != course_id {
                return Err(AppError::NotFound(
                    "course node does not belong to the course in this path".into(),
                ));
            }
            Ok(course_node_to_object(node))
        }
        VirtualPath::Root
        | VirtualPath::Sources
        | VirtualPath::SourceDocuments(_)
        | VirtualPath::Concepts
        | VirtualPath::Courses
        | VirtualPath::CourseNodes(_) => Err(AppError::InvalidInput(
            "virtual path points to a collection, not an object".into(),
        )),
    }
}

async fn resolve_source(pool: &SqlitePool, id: Uuid) -> Result<KnowledgeObject, AppError> {
    if let Ok(object) = knowledge_repo::get_object(pool, id).await {
        return Ok(object);
    }
    let source = source_repo::get_source(pool, id).await?;
    Ok(source_to_object(source))
}

async fn resolve_course(pool: &SqlitePool, id: Uuid) -> Result<KnowledgeObject, AppError> {
    if let Ok(object) = knowledge_repo::get_object(pool, id).await {
        return Ok(object);
    }
    let course = course_repo::get_course(pool, id).await?;
    Ok(KnowledgeObject {
        id: course.id,
        kind: KnowledgeObjectKind::Course,
        title: course.title,
        description: Some(course.description),
        metadata_json: course.metadata_json,
        created_at: course.created_at,
        updated_at: course.updated_at,
    })
}

pub fn source_to_object(source: Source) -> KnowledgeObject {
    let kind = match source.kind {
        SourceKind::File | SourceKind::Url | SourceKind::SearchResult => {
            KnowledgeObjectKind::Source
        }
    };
    KnowledgeObject {
        id: source.id,
        kind,
        title: source.title,
        description: source.uri,
        metadata_json: "{}".into(),
        created_at: source.created_at,
        updated_at: source.updated_at,
    }
}

pub fn course_node_to_object(node: crate::domain::CourseNode) -> KnowledgeObject {
    KnowledgeObject {
        id: node.id,
        kind: KnowledgeObjectKind::CourseNode,
        title: node.title,
        description: Some(node.objective),
        metadata_json: node.metadata_json,
        created_at: node.created_at,
        updated_at: node.updated_at,
    }
}

pub async fn chunks_for_object(
    pool: &SqlitePool,
    object_id: Uuid,
) -> Result<Vec<crate::domain::Chunk>, AppError> {
    let relations =
        knowledge_repo::query_relations(pool, Some(object_id), None, Some(RelationType::Contains))
            .await?;
    let mut output = Vec::new();
    for relation in relations {
        if let Ok(chunk) = chunk_repo::get_chunk(pool, relation.to_object_id).await {
            output.push(chunk);
        }
    }
    Ok(output)
}
