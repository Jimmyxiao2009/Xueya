use anyhow::Context;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;
use verruckt_cloud::config::Config;
use verruckt_cloud::db::connect_and_migrate;
use verruckt_cloud::services::ai::AiGateway;
use verruckt_cloud::state::AppState;
use verruckt_cloud_storage::S3ObjectStorage;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    init_tracing();

    let config = Config::from_env().context("invalid cloud configuration")?;
    tracing::info!(config = ?config, "starting verruckt-cloud");

    let pool = connect_with_retry(&config.database_url).await?;
    let storage = config
        .object_storage
        .clone()
        .map(S3ObjectStorage::new)
        .transpose()
        .context("invalid object storage configuration")?
        .map(Arc::new);

    if let Some(storage) = storage.as_ref() {
        ensure_bucket_with_retry(storage).await?;
    }

    let ai = config
        .ai
        .clone()
        .map(|ai_config| AiGateway::new(ai_config, config.ai_request_timeout()))
        .transpose()?
        .map(Arc::new);

    let state = AppState::new(pool, config.clone(), storage, ai);
    let router = verruckt_cloud::build_router(state);
    let address = std::net::SocketAddr::new(config.bind, config.port);
    let listener = TcpListener::bind(address)
        .await
        .with_context(|| format!("failed to bind {address}"))?;

    tracing::info!(%address, "verruckt-cloud is listening");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;
    Ok(())
}

async fn connect_with_retry(database_url: &str) -> anyhow::Result<sqlx::PgPool> {
    let mut last_error = None;
    for attempt in 1..=60 {
        match connect_and_migrate(database_url).await {
            Ok(pool) => return Ok(pool),
            Err(error) => {
                tracing::warn!(attempt, error = %error, "database not ready yet");
                last_error = Some(error);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("database connection timed out")))
}

async fn ensure_bucket_with_retry(storage: &S3ObjectStorage) -> anyhow::Result<()> {
    let mut last_error = None;
    for attempt in 1..=60 {
        match storage.ensure_bucket().await {
            Ok(()) => return Ok(()),
            Err(error) => {
                tracing::warn!(attempt, error = %error, "object storage not ready yet");
                last_error = Some(error);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
    Err(last_error
        .map(anyhow::Error::from)
        .unwrap_or_else(|| anyhow::anyhow!("object storage bucket timed out")))
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    if std::env::var("VERRUCKT_LOG_FORMAT")
        .map(|value| value.eq_ignore_ascii_case("json"))
        .unwrap_or(false)
    {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(filter)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
