use crate::db::repo::knowledge as knowledge_repo;
use crate::domain::{KnowledgeObject, KnowledgeRelation, RelationType};
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
