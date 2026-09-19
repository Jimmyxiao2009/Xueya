use crate::ai::registry::ProviderRegistry;
use crate::cloud::CloudAccountClient;
use crate::db::migrate;
use crate::error::AppError;
use crate::jobs::manager::JobManager;
use crate::knowledge::vector_index::{SqliteVectorIndex, VectorIndex};
use crate::knowledge::verruckt_vector::VerrucktVectorStoreAdapter;
use crate::knowledge_fs::blob_store::BlobStore;
use crate::search::SearchProviderRegistry;
use crate::secrets::keyring_store::{KeyringSecretStore, SecretStore};
use crate::sync::{CloudFileSyncService, CloudSyncService};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub app_data_dir: PathBuf,
    pub blobs_dir: PathBuf,
    pub db_path: PathBuf,
}

impl AppPaths {
    pub fn new(app_data_dir: impl Into<PathBuf>) -> Self {
        let app_data_dir = app_data_dir.into();
        Self {
            blobs_dir: app_data_dir.join("blobs"),
            db_path: app_data_dir.join("verruckt.sqlite"),
            app_data_dir,
        }
    }

    pub async fn ensure_dirs(&self) -> Result<(), AppError> {
        tokio::fs::create_dir_all(&self.app_data_dir).await?;
        tokio::fs::create_dir_all(&self.blobs_dir).await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorBackend {
    Sqlite,
    Verruckt,
}

impl VectorBackend {
    pub fn from_env() -> Self {
        match std::env::var("VERRUCKT_VECTOR_BACKEND")
            .ok()
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("verruckt") => Self::Verruckt,
            Some("sqlite") | None => Self::Sqlite,
            Some(value) => {
                tracing::warn!(
                    backend = value,
                    "unknown vector backend; falling back to sqlite"
                );
                Self::Sqlite
            }
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub http: reqwest::Client,
    pub secrets: Arc<dyn SecretStore>,
    pub cloud: Arc<CloudAccountClient>,
    pub sync: Arc<CloudSyncService>,
    pub providers: ProviderRegistry,
    pub search: SearchProviderRegistry,
    pub vector_index: Arc<dyn VectorIndex>,
    pub jobs: JobManager,
    pub paths: AppPaths,
    pub blobs: BlobStore,
    pub files: Arc<CloudFileSyncService>,
}

impl AppState {
    pub async fn new(paths: AppPaths) -> Result<Self, AppError> {
        Self::new_with_secrets(paths, Arc::new(KeyringSecretStore::new())).await
    }

    pub async fn new_with_secrets(
        paths: AppPaths,
        secrets: Arc<dyn SecretStore>,
    ) -> Result<Self, AppError> {
        Self::new_with_secrets_and_backend(paths, secrets, VectorBackend::from_env()).await
    }

    pub async fn new_with_secrets_and_backend(
        paths: AppPaths,
        secrets: Arc<dyn SecretStore>,
        backend: VectorBackend,
    ) -> Result<Self, AppError> {
        paths.ensure_dirs().await?;

        let connect_options = SqliteConnectOptions::new()
            .filename(&paths.db_path)
            .create_if_missing(true)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(10));
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(connect_options)
            .await?;
        migrate::run(&pool).await?;

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(180))
            .user_agent("Verruckt/0.1")
            .build()
            .map_err(|error| AppError::Network(error.to_string()))?;

        let cloud = Arc::new(CloudAccountClient::new(
            http.clone(),
            secrets.clone(),
            pool.clone(),
            crate::cloud::default_cloud_base_url(),
        ));
        cloud.initialize().await?;
        // A crash during a pull-apply can leave the guard set. Clearing it at
        // boot makes sure subsequent local writes are journaled again.
        sqlx::query("UPDATE sync_apply_guard SET active = 0 WHERE id = 1")
            .execute(&pool)
            .await?;
        let sync = Arc::new(CloudSyncService::new(cloud.clone(), pool.clone()));
        let providers = ProviderRegistry::new(pool.clone(), http.clone(), secrets.clone())
            .with_cloud(cloud.clone());
        let search = SearchProviderRegistry::new(http.clone());
        let vector_index: Arc<dyn VectorIndex> = match backend {
            VectorBackend::Sqlite => Arc::new(SqliteVectorIndex::new(pool.clone())),
            VectorBackend::Verruckt => Arc::new(
                VerrucktVectorStoreAdapter::new(pool.clone(), paths.app_data_dir.clone()).await?,
            ),
        };
        if let Err(error) = vector_index.rebuild().await {
            if backend == VectorBackend::Verruckt {
                tracing::warn!(%error, "verruckt vector index rebuild failed; retrying from SQLite on demand");
            } else {
                return Err(error);
            }
        }
        let jobs = JobManager::new(pool.clone());
        let blobs = BlobStore::new(paths.blobs_dir.clone());
        let files = Arc::new(CloudFileSyncService::new(
            cloud.clone(),
            pool.clone(),
            blobs.clone(),
            http.clone(),
        ));

        Ok(Self {
            pool,
            http,
            secrets,
            cloud,
            sync,
            providers,
            search,
            vector_index,
            jobs,
            paths,
            blobs,
            files,
        })
    }

    pub fn app_data_dir(&self) -> &Path {
        &self.paths.app_data_dir
    }
}
