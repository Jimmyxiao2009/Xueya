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
}
