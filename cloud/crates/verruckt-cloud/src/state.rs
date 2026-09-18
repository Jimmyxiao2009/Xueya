use crate::config::Config;
use crate::services::ai::AiGateway;
use crate::services::rate_limit::RateLimiter;
use sqlx::PgPool;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use verruckt_cloud_storage::S3ObjectStorage;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub storage: Option<Arc<S3ObjectStorage>>,
    pub ai: Option<Arc<AiGateway>>,
    pub rate_limiter: Arc<RateLimiter>,
    pub provider_status: Arc<ProviderStatus>,
}

impl AppState {
    pub fn new(
        pool: PgPool,
        config: Config,
        storage: Option<Arc<S3ObjectStorage>>,
        ai: Option<Arc<AiGateway>>,
    ) -> Self {
        let rate_limiter = Arc::new(RateLimiter::new(config.ai_rate_limit_per_minute));
        Self {
            pool,
            config: Arc::new(config),
            storage,
            ai,
            rate_limiter,
            provider_status: Arc::new(ProviderStatus::default()),
        }
    }
}

#[derive(Debug, Default)]
pub struct ProviderStatus {
    ai_ok: AtomicBool,
    ai_checked: AtomicBool,
}

impl ProviderStatus {
    pub fn mark_ok(&self) {
        self.ai_checked.store(true, Ordering::Relaxed);
        self.ai_ok.store(true, Ordering::Relaxed);
    }

    pub fn mark_degraded(&self) {
        self.ai_checked.store(true, Ordering::Relaxed);
        self.ai_ok.store(false, Ordering::Relaxed);
    }

    pub fn ai_status(&self) -> &'static str {
        if !self.ai_checked.load(Ordering::Relaxed) {
            "unknown"
        } else if self.ai_ok.load(Ordering::Relaxed) {
            "ok"
        } else {
            "degraded"
        }
    }
}
