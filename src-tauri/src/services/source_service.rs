use crate::db::repo::sources as source_repo;
use crate::domain::Source;
use crate::error::AppError;
use sqlx::SqlitePool;
use uuid::Uuid;

pub async fn list_sources(pool: &SqlitePool) -> Result<Vec<Source>, AppError> {
    source_repo::list_sources(pool).await
}

pub async fn get_source(pool: &SqlitePool, id: Uuid) -> Result<Source, AppError> {
    source_repo::get_source(pool, id).await
}

pub async fn delete_source(pool: &SqlitePool, id: Uuid) -> Result<(), AppError> {
    source_repo::delete_source(pool, id).await
}
