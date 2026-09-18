use crate::error::AppError;
use sqlx::SqlitePool;

pub async fn run(pool: &SqlitePool) -> Result<(), AppError> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|error| AppError::Database(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqliteConnectOptions;
    use sqlx::SqlitePool;
    use std::str::FromStr;

    #[tokio::test]
    async fn migrations_apply_to_fresh_database() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("sqlite options")
            .foreign_keys(true);
        let pool = SqlitePool::connect_with(options).await.expect("pool");
        run(&pool).await.expect("migrate");

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM providers")
            .fetch_one(&pool)
            .await
            .expect("providers table exists");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn local_ai_core_migration_is_additive() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("sqlite options")
            .foreign_keys(true);
        let pool = SqlitePool::connect_with(options).await.expect("pool");
        run(&pool).await.expect("migrate");

        // Old rows get the new protocol default without a data migration.
        sqlx::query(
            "INSERT INTO providers (id, display_name, base_url, chat_model, embedding_model, enabled, created_at, updated_at) \
             VALUES ('00000000-0000-0000-0000-000000000099', 'Old', 'https://example.com', 'm', 'e', 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("old-shaped insert");
        let protocol: String = sqlx::query_scalar(
            "SELECT protocol FROM providers WHERE id = '00000000-0000-0000-0000-000000000099'",
        )
        .fetch_one(&pool)
        .await
        .expect("protocol");
        assert_eq!(protocol, "auto");

        for table in [
            "chunk_fts",
            "embedding_index_metadata",
            "source_index_state",
            "embedding_cache",
            "source_summaries",
            "session_summaries",
            "usage_events",
            "provider_capability_cache",
        ] {
            let exists: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sqlite_master WHERE type IN ('table','view') AND name = ?",
            )
            .bind(table)
            .fetch_one(&pool)
            .await
            .expect("schema query");
            assert_eq!(exists, 1, "missing table {table}");
        }

        for column in [
            "source_id",
            "route_id",
            "text_hash",
            "metadata_json",
            "embedding_status",
        ] {
            let found: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM pragma_table_info('chunks') WHERE name = ?",
            )
            .bind(column)
            .fetch_one(&pool)
            .await
            .expect("pragma");
            assert_eq!(found, 1, "missing chunks.{column}");
        }
    }
}
