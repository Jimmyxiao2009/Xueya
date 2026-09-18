use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub async fn connect_pool(database_url: &str) -> Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(database_url)
        .await
        .context("failed to connect to PostgreSQL")
}

pub async fn connect_and_migrate(database_url: &str) -> Result<PgPool> {
    let pool = connect_pool(database_url).await?;
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .context("failed to run cloud migrations")?;
    Ok(pool)
}
