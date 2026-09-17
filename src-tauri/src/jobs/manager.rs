use super::cancellation::CancellationToken;
use super::event::{JobEvent, JobEventKind};
use crate::db::repo::jobs as job_repo;
use crate::domain::{Job, JobStatus};
use crate::error::AppError;
use crate::util::time::utc_now;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

struct JobEntry {
    job: Mutex<Job>,
    token: CancellationToken,
}

struct Inner {
    entries: RwLock<HashMap<Uuid, Arc<JobEntry>>>,
    senders: Mutex<HashMap<Uuid, broadcast::Sender<JobEvent>>>,
}

#[derive(Clone)]
pub struct JobManager {
    pool: SqlitePool,
    inner: Arc<Inner>,
}

#[derive(Clone)]
pub struct JobHandle {
    id: Uuid,
    manager: JobManager,
    token: CancellationToken,
}

impl JobHandle {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.token.clone()
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token().is_cancelled()
    }

    pub async fn progress(
        &self,
        progress: f64,
        message: impl Into<String>,
    ) -> Result<(), AppError> {
        self.manager
            .update_progress(self.id, progress, message.into(), JobEventKind::Progress)
            .await
    }

    pub async fn partial(&self, message: impl Into<String>) -> Result<(), AppError> {
        let current = self
            .manager
            .snapshot(self.id)
            .await
            .map(|job| job.progress)
            .unwrap_or(0.0);
        self.manager
            .update_progress(self.id, current, message.into(), JobEventKind::Partial)
            .await
    }
}

impl JobManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            inner: Arc::new(Inner {
                entries: RwLock::new(HashMap::new()),
                senders: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub async fn start<F, Fut>(&self, kind: impl Into<String>, task: F) -> Result<Job, AppError>
    where
        F: FnOnce(JobHandle) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), AppError>> + Send + 'static,
    {
        let id = Uuid::new_v4();
        let job = Job {
            id,
            kind: kind.into(),
            status: JobStatus::Queued,
            progress: 0.0,
            message: "Queued".into(),
            error_json: None,
            created_at: utc_now(),
            started_at: None,
            completed_at: None,
        };
        job_repo::insert_job(&self.pool, &job).await?;
        let (sender, _) = broadcast::channel(128);
        self.inner
            .senders
            .lock()
            .map_err(|_| AppError::Internal("job sender registry poisoned".into()))?
            .insert(id, sender);
        let entry = Arc::new(JobEntry {
            job: Mutex::new(job.clone()),
            token: CancellationToken::new(),
        });
        self.inner.entries.write().await.insert(id, entry.clone());
        let manager = self.clone();
        let token = entry.token.clone();
        tokio::spawn(async move {
            let handle = JobHandle {
                id,
                manager: manager.clone(),
                token: token.clone(),
            };
            if entry.token.is_cancelled() {
                let _ = manager
                    .set_terminal(id, JobStatus::Cancelled, "Cancelled", None)
                    .await;
                return;
            }
            if let Err(error) = manager
                .update_status(
                    id,
                    JobStatus::Running,
                    0.0,
                    "Running",
                    None,
                    Some(utc_now()),
                )
                .await
            {
                let _ = manager
                    .set_terminal(id, JobStatus::Failed, "Failed", Some(error))
                    .await;
                return;
            }
            let result = task(handle.clone()).await;
            if entry.token.is_cancelled() {
                let _ = manager
                    .set_terminal(id, JobStatus::Cancelled, "Cancelled", None)
                    .await;
                return;
            }
            match result {
                Ok(()) => {
                    let _ = manager
                        .set_terminal(id, JobStatus::Completed, "Completed", None)
                        .await;
                }
                Err(AppError::JobCancelled) => {
                    let _ = manager
                        .set_terminal(id, JobStatus::Cancelled, "Cancelled", None)
                        .await;
                }
                Err(error) => {
                    let _ = manager
                        .set_terminal(id, JobStatus::Failed, "Failed", Some(error))
                        .await;
                }
            }
        });
        Ok(job)
    }

    pub async fn cancel(&self, id: Uuid) -> Result<Job, AppError> {
        let entry = self.entry(id).await?;
        entry.token.cancel();
        self.set_terminal(id, JobStatus::Cancelled, "Cancelled", None)
            .await?;
        self.snapshot(id)
            .await
            .ok_or_else(|| AppError::NotFound(format!("job {id}")))
    }

    pub async fn list(&self) -> Result<Vec<Job>, AppError> {
        job_repo::list_jobs(&self.pool).await
    }

    pub async fn get(&self, id: Uuid) -> Result<Job, AppError> {
        job_repo::get_job(&self.pool, id).await
    }

    pub async fn subscribe(&self, id: Uuid) -> Result<broadcast::Receiver<JobEvent>, AppError> {
        self.inner
            .senders
            .lock()
            .map_err(|_| AppError::Internal("job sender registry poisoned".into()))?
            .get(&id)
            .map(broadcast::Sender::subscribe)
            .ok_or_else(|| AppError::NotFound(format!("job {id}")))
    }

    async fn snapshot(&self, id: Uuid) -> Option<Job> {
        let entry = self.inner.entries.read().await.get(&id).cloned()?;
        entry.job.lock().ok().map(|job| job.clone())
    }

    async fn entry(&self, id: Uuid) -> Result<Arc<JobEntry>, AppError> {
        self.inner
            .entries
            .read()
            .await
            .get(&id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("job {id}")))
    }

    async fn update_progress(
        &self,
        id: Uuid,
        progress: f64,
        message: String,
        event_kind: JobEventKind,
    ) -> Result<(), AppError> {
        let entry = self.entry(id).await?;
        let job = {
            let mut guard = entry
                .job
                .lock()
                .map_err(|_| AppError::Internal("job state poisoned".into()))?;
            guard.progress = progress.clamp(0.0, 1.0);
            guard.message = message.clone();
            guard.clone()
        };
        job_repo::update_job(
            &self.pool,
            id,
            job.status,
            job.progress,
            &job.message,
            job.error_json.as_deref(),
            job.started_at,
            job.completed_at,
        )
        .await?;
        self.publish(id, job, event_kind, None);
        Ok(())
    }

    async fn update_status(
        &self,
        id: Uuid,
        status: JobStatus,
        progress: f64,
        message: &str,
        error: Option<AppError>,
        started_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), AppError> {
        self.apply(
            id,
            status,
            progress,
            message,
            error,
            started_at,
            None,
            match status {
                JobStatus::Running => JobEventKind::Started,
                _ => JobEventKind::Progress,
            },
        )
        .await
    }

    async fn set_terminal(
        &self,
        id: Uuid,
        status: JobStatus,
        message: &str,
        error: Option<AppError>,
    ) -> Result<(), AppError> {
        let event_kind = match status {
            JobStatus::Completed => JobEventKind::Completed,
            JobStatus::Failed => JobEventKind::Failed,
            JobStatus::Cancelled => JobEventKind::Cancelled,
            JobStatus::Queued | JobStatus::Running => JobEventKind::Progress,
        };
        self.apply(
            id,
            status,
            1.0,
            message,
            error,
            None,
            Some(utc_now()),
            event_kind,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn apply(
        &self,
        id: Uuid,
        status: JobStatus,
        progress: f64,
        message: &str,
        error: Option<AppError>,
        started_at: Option<chrono::DateTime<chrono::Utc>>,
        completed_at: Option<chrono::DateTime<chrono::Utc>>,
        event_kind: JobEventKind,
    ) -> Result<(), AppError> {
        let entry = self.entry(id).await?;
        let job = {
            let mut guard = entry
                .job
                .lock()
                .map_err(|_| AppError::Internal("job state poisoned".into()))?;
            guard.status = status;
            guard.progress = progress.clamp(0.0, 1.0);
            guard.message = message.to_owned();
            guard.error_json = error
                .as_ref()
                .and_then(|value| serde_json::to_string(value).ok());
            if started_at.is_some() {
                guard.started_at = started_at;
            }
            if completed_at.is_some() {
                guard.completed_at = completed_at;
            }
            guard.clone()
        };
        job_repo::update_job(
            &self.pool,
            id,
            job.status,
            job.progress,
            &job.message,
            job.error_json.as_deref(),
            job.started_at,
            job.completed_at,
        )
        .await?;
        self.publish(id, job, event_kind, error);
        Ok(())
    }

    fn publish(&self, id: Uuid, job: Job, event_kind: JobEventKind, error: Option<AppError>) {
        let event = JobEvent {
            job_id: id,
            event: event_kind,
            status: job.status,
            progress: job.progress,
            message: job.message,
            error: error
                .as_ref()
                .and_then(|value| serde_json::to_value(value).ok()),
        };
        if let Ok(senders) = self.inner.senders.lock() {
            if let Some(sender) = senders.get(&id) {
                let _ = sender.send(event);
            }
        }
    }
}
