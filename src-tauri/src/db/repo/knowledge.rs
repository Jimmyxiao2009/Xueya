use crate::domain::{KnowledgeObject, KnowledgeRelation, Note, RelationType};
use crate::error::AppError;
use sqlx::SqlitePool;
use uuid::Uuid;

pub async fn upsert_object(pool: &SqlitePool, object: &KnowledgeObject) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO knowledge_objects
            (id, kind, title, description, metadata_json, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            kind = excluded.kind,
            title = excluded.title,
            description = excluded.description,
            metadata_json = excluded.metadata_json,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(object.id)
    .bind(object.kind)
    .bind(&object.title)
    .bind(&object.description)
    .bind(&object.metadata_json)
    .bind(object.created_at.to_rfc3339())
    .bind(object.updated_at.to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_relation(
    pool: &SqlitePool,
    relation: &KnowledgeRelation,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO knowledge_relations
            (id, from_object_id, to_object_id, relation_type, weight, metadata_json, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(from_object_id, to_object_id, relation_type) DO UPDATE SET
            weight = excluded.weight,
            metadata_json = excluded.metadata_json
        "#,
    )
    .bind(relation.id)
    .bind(relation.from_object_id)
    .bind(relation.to_object_id)
    .bind(relation.relation_type)
    .bind(relation.weight)
    .bind(&relation.metadata_json)
    .bind(relation.created_at.to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_object(pool: &SqlitePool, id: Uuid) -> Result<KnowledgeObject, AppError> {
    sqlx::query_as::<_, KnowledgeObject>(
        "SELECT id, kind, title, description, metadata_json, created_at, updated_at FROM knowledge_objects WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("knowledge object {id}")))
}

pub async fn query_relations(
    pool: &SqlitePool,
    from_object_id: Option<Uuid>,
    to_object_id: Option<Uuid>,
    relation_type: Option<RelationType>,
) -> Result<Vec<KnowledgeRelation>, AppError> {
    let mut sql = String::from(
        "SELECT id, from_object_id, to_object_id, relation_type, weight, metadata_json, created_at FROM knowledge_relations WHERE 1=1",
    );
    if from_object_id.is_some() {
        sql.push_str(" AND from_object_id = ?");
    }
    if to_object_id.is_some() {
        sql.push_str(" AND to_object_id = ?");
    }
    if relation_type.is_some() {
        sql.push_str(" AND relation_type = ?");
    }
    sql.push_str(" ORDER BY created_at DESC");
    let mut query = sqlx::query_as::<_, KnowledgeRelation>(&sql);
    if let Some(value) = from_object_id {
        query = query.bind(value);
    }
    if let Some(value) = to_object_id {
        query = query.bind(value);
    }
    if let Some(value) = relation_type {
        query = query.bind(value);
    }
    Ok(query.fetch_all(pool).await?)
}

pub async fn get_related(
    pool: &SqlitePool,
    object_id: Uuid,
    relation_type: Option<RelationType>,
) -> Result<Vec<KnowledgeRelation>, AppError> {
    let mut sql = String::from(
        "SELECT id, from_object_id, to_object_id, relation_type, weight, metadata_json, created_at FROM knowledge_relations WHERE (from_object_id = ? OR to_object_id = ?)",
    );
    if relation_type.is_some() {
        sql.push_str(" AND relation_type = ?");
    }
    sql.push_str(" ORDER BY created_at DESC");
    let mut query = sqlx::query_as::<_, KnowledgeRelation>(&sql)
        .bind(object_id)
        .bind(object_id);
    if let Some(value) = relation_type {
        query = query.bind(value);
    }
    Ok(query.fetch_all(pool).await?)
}

pub async fn upsert_note(pool: &SqlitePool, note: &Note) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO notes (id, object_id, content, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at
        "#,
    )
    .bind(note.id)
    .bind(note.object_id)
    .bind(&note.content)
    .bind(note.created_at.to_rfc3339())
    .bind(note.updated_at.to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}
