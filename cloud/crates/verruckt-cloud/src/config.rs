use anyhow::{anyhow, Context, Result};
use std::env;
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::Duration;
use verruckt_cloud_storage::ObjectStorageConfig;

#[derive(Clone)]
pub struct Config {
    pub environment: String,
    pub bind: IpAddr,
    pub port: u16,
    pub database_url: String,
    pub access_token_secret: Vec<u8>,
    pub token_issuer: String,
    pub access_token_ttl_secs: i64,
    pub refresh_token_ttl_days: i64,
    pub request_timeout_secs: u64,
    pub max_body_bytes: usize,
    pub cors_origins: Vec<String>,
    pub object_storage: Option<ObjectStorageConfig>,
    pub ai: Option<AiConfig>,
    pub ai_rate_limit_per_minute: u32,
    pub ai_request_timeout_secs: u64,
    pub max_file_bytes: i64,
}

#[derive(Clone)]
pub struct AiConfig {
    pub base_url: String,
    pub api_key: String,
    pub models: std::collections::BTreeMap<String, String>,
    pub max_messages: usize,
    pub max_tokens: u32,
    pub max_embedding_inputs: usize,
    pub max_embedding_total_chars: usize,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("environment", &self.environment)
            .field("bind", &self.bind)
            .field("port", &self.port)
            .field("database_url", &"<redacted>")
            .field("access_token_secret", &"<redacted>")
            .field("token_issuer", &self.token_issuer)
            .field("access_token_ttl_secs", &self.access_token_ttl_secs)
            .field("refresh_token_ttl_days", &self.refresh_token_ttl_days)
            .field("request_timeout_secs", &self.request_timeout_secs)
            .field("max_body_bytes", &self.max_body_bytes)
            .field("cors_origins", &self.cors_origins)
            .field(
                "object_storage",
                &self.object_storage.as_ref().map(|_| "<configured>"),
            )
            .field("ai", &self.ai.as_ref().map(|_| "<configured>"))
            .field("ai_rate_limit_per_minute", &self.ai_rate_limit_per_minute)
            .field("ai_request_timeout_secs", &self.ai_request_timeout_secs)
            .field("max_file_bytes", &self.max_file_bytes)
            .finish()
    }
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let environment = env::var("VERRUCKT_ENV").unwrap_or_else(|_| "development".into());
        let production = environment.eq_ignore_ascii_case("production");

        let bind = env::var("VERRUCKT_BIND")
            .unwrap_or_else(|_| "0.0.0.0".into())
            .parse::<IpAddr>()
            .context("VERRUCKT_BIND must be an IP address")?;
        let port = parse_or("VERRUCKT_PORT", 8080u16)?;

        let database_url = required_secret_or_default(
            "DATABASE_URL",
            "postgres://verruckt:verruckt@localhost:5432/verruckt",
            production,
        )?;

        let access_token_secret = required_secret_or_default(
            "VERRUCKT_ACCESS_TOKEN_SECRET",
            "dev-only-change-me-access-secret",
            production,
        )?
        .into_bytes();

        let object_storage = object_storage_from_env()?;
        let ai = ai_from_env()?;

        let cors_origins = env::var("VERRUCKT_CORS_ORIGINS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();

        Ok(Self {
            environment,
            bind,
            port,
            database_url,
            access_token_secret,
            token_issuer: env::var("VERRUCKT_TOKEN_ISSUER")
                .unwrap_or_else(|_| "verruckt-cloud".into()),
            access_token_ttl_secs: parse_or("VERRUCKT_ACCESS_TOKEN_TTL_SECS", 1800i64)?,
            refresh_token_ttl_days: parse_or("VERRUCKT_REFRESH_TOKEN_TTL_DAYS", 30i64)?,
            request_timeout_secs: parse_or("VERRUCKT_REQUEST_TIMEOUT_SECS", 30u64)?,
            max_body_bytes: parse_or("VERRUCKT_MAX_BODY_BYTES", 4 * 1024 * 1024usize)?,
            cors_origins,
            object_storage,
            ai,
            ai_rate_limit_per_minute: parse_or("VERRUCKT_AI_RATE_LIMIT_PER_MINUTE", 60u32)?,
            ai_request_timeout_secs: parse_or("VERRUCKT_AI_REQUEST_TIMEOUT_SECS", 120u64)?,
            max_file_bytes: parse_or("VERRUCKT_MAX_FILE_BYTES", 2 * 1024 * 1024 * 1024i64)?,
        })
    }

    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.request_timeout_secs)
    }

    pub fn ai_request_timeout(&self) -> Duration {
        Duration::from_secs(self.ai_request_timeout_secs)
    }

    pub fn access_token_ttl(&self) -> chrono::Duration {
        chrono::Duration::seconds(self.access_token_ttl_secs)
    }

    pub fn refresh_token_ttl(&self) -> chrono::Duration {
        chrono::Duration::days(self.refresh_token_ttl_days)
    }
}

fn object_storage_from_env() -> Result<Option<ObjectStorageConfig>> {
    let endpoint = env::var("VERRUCKT_OBJECT_STORAGE_ENDPOINT").unwrap_or_default();
    if endpoint.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(ObjectStorageConfig {
        endpoint,
        region: env::var("VERRUCKT_OBJECT_STORAGE_REGION").unwrap_or_else(|_| "us-east-1".into()),
        bucket: env::var("VERRUCKT_OBJECT_STORAGE_BUCKET")
            .unwrap_or_else(|_| "verruckt-cloud".into()),
        access_key: env_or_file("VERRUCKT_OBJECT_STORAGE_ACCESS_KEY")?
            .unwrap_or_else(|| "minioadmin".into()),
        secret_key: env_or_file("VERRUCKT_OBJECT_STORAGE_SECRET_KEY")?
            .unwrap_or_else(|| "minioadmin".into()),
        force_path_style: parse_or("VERRUCKT_OBJECT_STORAGE_FORCE_PATH_STYLE", true)?,
        presign_ttl_secs: parse_or("VERRUCKT_OBJECT_STORAGE_PRESIGN_TTL_SECS", 900u64)?,
    }))
}

fn ai_from_env() -> Result<Option<AiConfig>> {
    let base_url = env::var("VERRUCKT_UPSTREAM_BASE_URL").unwrap_or_default();
    if base_url.trim().is_empty() {
        return Ok(None);
    }

    let api_key = env_or_file("VERRUCKT_UPSTREAM_API_KEY")?
        .ok_or_else(|| anyhow!("VERRUCKT_UPSTREAM_API_KEY or VERRUCKT_UPSTREAM_API_KEY_FILE is required when upstream AI is configured"))?;

    let chat_model =
        env::var("VERRUCKT_UPSTREAM_CHAT_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());
    let reasoning_model =
        env::var("VERRUCKT_UPSTREAM_REASONING_MODEL").unwrap_or_else(|_| chat_model.clone());
    let embedding_model = env::var("VERRUCKT_UPSTREAM_EMBEDDING_MODEL")
        .unwrap_or_else(|_| "text-embedding-3-small".into());

    let mut models = std::collections::BTreeMap::new();
    models.insert(
        "verruckt-fast".into(),
        env::var("VERRUCKT_MODEL_FAST").unwrap_or_else(|_| chat_model.clone()),
    );
    models.insert(
        "verruckt-standard".into(),
        env::var("VERRUCKT_MODEL_STANDARD").unwrap_or_else(|_| chat_model.clone()),
    );
    models.insert(
        "verruckt-reasoning".into(),
        env::var("VERRUCKT_MODEL_REASONING").unwrap_or_else(|_| reasoning_model.clone()),
    );
    models.insert(
        "verruckt-embedding".into(),
        env::var("VERRUCKT_MODEL_EMBEDDING").unwrap_or_else(|_| embedding_model.clone()),
    );

    Ok(Some(AiConfig {
        base_url,
        api_key,
        models,
        max_messages: parse_or("VERRUCKT_AI_MAX_MESSAGES", 64usize)?,
        max_tokens: parse_or("VERRUCKT_AI_MAX_TOKENS", 8192u32)?,
        max_embedding_inputs: parse_or("VERRUCKT_AI_MAX_EMBEDDING_INPUTS", 128usize)?,
        max_embedding_total_chars: parse_or(
            "VERRUCKT_AI_MAX_EMBEDDING_TOTAL_CHARS",
            128 * 1024usize,
        )?,
    }))
}

fn parse_or<T>(name: &str, default: T) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => value
            .parse::<T>()
            .map_err(|error| anyhow!("{name} is invalid: {error}")),
        _ => Ok(default),
    }
}

fn required_secret_or_default(name: &str, default: &str, production: bool) -> Result<String> {
    match env_or_file(name)? {
        Some(value) => Ok(value),
        None if production => Err(anyhow!("{name} or {name}_FILE is required in production")),
        None => {
            tracing::warn!(variable = name, "using development-only default secret");
            Ok(default.into())
        }
    }
}

fn env_or_file(name: &str) -> Result<Option<String>> {
    if let Ok(path) = env::var(format!("{name}_FILE")) {
        if !path.trim().is_empty() {
            let value = std::fs::read_to_string(PathBuf::from(&path))
                .with_context(|| format!("failed to read {name}_FILE"))?;
            let value = value.trim().to_string();
            if !value.is_empty() {
                return Ok(Some(value));
            }
        }
    }
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        _ => Ok(None),
    }
}
