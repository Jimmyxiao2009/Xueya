use super::models::{
    CloudAccountState, CloudAiUsage, CloudAuthResponse, CloudDevice, CloudDeviceDto,
    CloudDeviceInput, CloudEntitlements, CloudEntitlementsDto, CloudErrorEnvelope,
    CloudLoginRequest, CloudLogoutRequest, CloudMeResponse, CloudRefreshRequest,
    CloudRegisterRequest, CloudSession,
};
use crate::error::AppError;
use crate::secrets::keyring_store::SecretStore;
use crate::util::time::utc_now;
use reqwest::{Method, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use sqlx::SqlitePool;
use std::sync::{Arc, RwLock};
use tokio::sync::{broadcast, Mutex as AsyncMutex, RwLock as AsyncRwLock};
use uuid::Uuid;

/// A cloud session event that is safe to forward to the WebView. The core
/// client does not depend on Tauri; `lib.rs::run` forwards these events.
#[derive(Debug, Clone)]
pub struct CloudEvent {
    pub event: String,
    pub payload: Value,
}

pub const REFRESH_TOKEN_REF: &str = "secret://cloud/refresh-token";
const DEVICE_ID_KEY: &str = "cloud_device_id";
const DEVICE_NAME_KEY: &str = "cloud_device_name";
const DEVICE_PLATFORM_KEY: &str = "cloud_device_platform";
const BASE_URL_KEY: &str = "cloud_base_url";
const SYNC_ENABLED_KEY: &str = "cloud_sync_enabled";

pub struct CloudAccountClient {
    http: reqwest::Client,
    secrets: Arc<dyn SecretStore>,
    pool: SqlitePool,
    base_url: RwLock<String>,
    session: AsyncRwLock<CloudSession>,
    refresh_lock: AsyncMutex<()>,
    events: broadcast::Sender<CloudEvent>,
}

impl CloudAccountClient {
    pub fn new(
        http: reqwest::Client,
        secrets: Arc<dyn SecretStore>,
        pool: SqlitePool,
        base_url: impl Into<String>,
    ) -> Self {
        let (events, _) = broadcast::channel(32);
        Self {
            http,
            secrets,
            pool,
            base_url: RwLock::new(base_url.into()),
            session: AsyncRwLock::new(CloudSession::default()),
            refresh_lock: AsyncMutex::new(()),
            events,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<CloudEvent> {
        self.events.subscribe()
    }

    pub async fn initialize(&self) -> Result<(), AppError> {
        if let Some(value) = self.setting_string(BASE_URL_KEY).await? {
            if let Ok(normalized) = normalize_cloud_base_url(&value) {
                *self.base_url.write().expect("base url lock") = normalized;
            }
        }
        Ok(())
    }

    pub fn base_url(&self) -> String {
        self.base_url
            .read()
            .map(|value| value.clone())
            .unwrap_or_else(|_| default_cloud_base_url())
    }

    pub async fn set_base_url(&self, value: &str) -> Result<CloudAccountState, AppError> {
        let normalized = normalize_cloud_base_url(value)?;
        *self.base_url.write().expect("base url lock") = normalized.clone();
        self.set_setting_string(BASE_URL_KEY, &normalized).await?;
        self.clear_session().await;
        self.account_state().await
    }

    pub async fn sync_enabled(&self) -> Result<bool, AppError> {
        Ok(self
            .setting_string(SYNC_ENABLED_KEY)
            .await?
            .and_then(|value| value.parse::<bool>().ok())
            .unwrap_or(false))
    }

    pub async fn set_sync_enabled(&self, enabled: bool) -> Result<CloudAccountState, AppError> {
        self.set_setting_string(SYNC_ENABLED_KEY, if enabled { "true" } else { "false" })
            .await?;
        self.emit(
            "cloud_sync_setting_changed",
            serde_json::json!({ "enabled": enabled }),
        );
        self.account_state().await
    }

    /// Return a currently valid access token, refreshing in the background if
    /// the short-lived token has expired. This is the only path that reads the
    /// refresh token, and it is never exposed to the frontend.
    pub async fn access_token(&self) -> Result<String, AppError> {
        {
            let session = self.session.read().await;
            if session.has_valid_access_token() {
                return Ok(session
                    .access_token
                    .clone()
                    .expect("valid token is present"));
            }
        }
        self.refresh_session().await?;
        let session = self.session.read().await;
        session
            .access_token
            .clone()
            .ok_or(AppError::CloudSessionExpired)
    }

    /// Force a refresh after a 401. Callers should retry the original request
    /// exactly once.
    pub async fn force_refresh(&self) -> Result<(), AppError> {
        {
            let mut session = self.session.write().await;
            session.access_token = None;
            session.access_expires_at = None;
        }
        self.refresh_session().await
    }

    async fn refresh_session(&self) -> Result<(), AppError> {
        let _guard = self.refresh_lock.lock().await;
        {
            let session = self.session.read().await;
            if session.has_valid_access_token() {
                return Ok(());
            }
        }

        let refresh_token = match self.secrets.get_secret(REFRESH_TOKEN_REF).await {
            Ok(token) => token,
            Err(AppError::NotFound(_)) => {
                self.clear_session().await;
                return Err(AppError::CloudSessionExpired);
            }
            Err(error) => return Err(error),
        };
        let body = CloudRefreshRequest {
            refresh_token,
            device: Some(self.device_input().await?),
        };
        let response = self
            .http
            .post(format!("{}/api/v1/auth/refresh", self.base_url()))
            .json(&body)
            .send()
            .await
            .map_err(network_error)?;

        if response.status() == StatusCode::UNAUTHORIZED {
            let error = self.map_response(response).await;
            self.clear_session().await;
            return Err(error);
        }
        if !response.status().is_success() {
            return Err(self.map_response(response).await);
        }
        let auth: CloudAuthResponse = response
            .json()
            .await
            .map_err(|error| AppError::CloudUnavailable(error.to_string()))?;
        self.store_auth(&auth).await?;
        Ok(())
    }

    pub async fn register(
        &self,
        email: String,
        password: String,
    ) -> Result<CloudAccountState, AppError> {
        validate_credentials(&email, &password)?;
        let body = serde_json::to_value(CloudRegisterRequest {
            email: email.trim().to_owned(),
            password,
            device: Some(self.device_input().await?),
        })
        .map_err(|error| AppError::Internal(error.to_string()))?;
        let response = self.public_json("/api/v1/auth/register", &body).await?;
        let auth: CloudAuthResponse = response
            .json()
            .await
            .map_err(|error| AppError::CloudUnavailable(error.to_string()))?;
        self.store_auth(&auth).await?;
        self.account_state().await
    }

    pub async fn login(
        &self,
        email: String,
        password: String,
    ) -> Result<CloudAccountState, AppError> {
        validate_credentials(&email, &password)?;
        let body = serde_json::to_value(CloudLoginRequest {
            email: email.trim().to_owned(),
            password,
            device: Some(self.device_input().await?),
            device_id: self.device_id().await?,
        })
        .map_err(|error| AppError::Internal(error.to_string()))?;
        let response = self.public_json("/api/v1/auth/login", &body).await?;
        let auth: CloudAuthResponse = response
            .json()
            .await
            .map_err(|error| AppError::CloudUnavailable(error.to_string()))?;
        self.store_auth(&auth).await?;
        self.account_state().await
    }

    pub async fn logout(&self) -> Result<CloudAccountState, AppError> {
        let refresh_token = self.secrets.get_secret(REFRESH_TOKEN_REF).await.ok();
        let body = serde_json::to_value(CloudLogoutRequest { refresh_token })
            .map_err(|error| AppError::Internal(error.to_string()))?;
        let _ = self.public_json("/api/v1/auth/logout", &body).await;
        self.clear_session().await;
        self.account_state().await
    }

    pub async fn account_state(&self) -> Result<CloudAccountState, AppError> {
        if !self.restore_if_needed().await? {
            return self.anonymous_state().await;
        }
        self.current_state().await
    }

    async fn restore_if_needed(&self) -> Result<bool, AppError> {
        {
            let session = self.session.read().await;
            if session.user.is_some() && session.entitlements.is_some() {
                return Ok(true);
            }
        }
        if !self.secrets.secret_exists(REFRESH_TOKEN_REF).await? {
            return Ok(false);
        }
        if let Err(error) = self.access_token().await {
            tracing::warn!(%error, "could not restore the Verruckt Cloud session");
            self.clear_session().await;
            return Ok(false);
        }
        // Populate the non-secret account state.
        if let Err(error) = self.refresh_me().await {
            tracing::warn!(%error, "could not load the Verruckt Cloud account");
            self.clear_session().await;
            return Ok(false);
        }
        if let Err(error) = self.refresh_entitlements().await {
            tracing::warn!(%error, "could not load Verruckt Cloud entitlements");
            self.clear_session().await;
            return Ok(false);
        }
        Ok(true)
    }

    async fn anonymous_state(&self) -> Result<CloudAccountState, AppError> {
        Ok(CloudAccountState {
            logged_in: false,
            email: None,
            user_id: None,
            plan: None,
            entitlements: None,
            device: None,
            sync_enabled: self.sync_enabled().await?,
            base_url: self.base_url(),
        })
    }

    async fn current_state(&self) -> Result<CloudAccountState, AppError> {
        let session = self.session.read().await;
        let entitlements = session
            .entitlements
            .as_ref()
            .map(CloudEntitlementsDto::from);
        Ok(CloudAccountState {
            logged_in: session.user.is_some(),
            email: session.user.as_ref().map(|user| user.email.clone()),
            user_id: session.user.as_ref().map(|user| user.id),
            plan: session
                .entitlements
                .as_ref()
                .map(|value| value.plan.clone()),
            entitlements,
            device: session.device.clone().map(CloudDeviceDto::from),
            sync_enabled: self.sync_enabled().await?,
            base_url: self.base_url(),
        })
    }

    pub async fn me(&self) -> Result<CloudMeResponse, AppError> {
        self.refresh_me().await?;
        let session = self.session.read().await;
        Ok(CloudMeResponse {
            user: session.user.clone().ok_or(AppError::CloudSessionExpired)?,
        })
    }

    pub async fn entitlements(&self) -> Result<CloudEntitlements, AppError> {
        self.refresh_entitlements().await?;
        let session = self.session.read().await;
        session
            .entitlements
            .clone()
            .ok_or(AppError::CloudSessionExpired)
    }

    /// Use the in-memory entitlement snapshot when one is available. AI calls
    /// use this to avoid a `GET /me/entitlements` before every request; the
    /// gateway still enforces the real quota server-side.
    pub async fn cached_entitlements(&self) -> Result<CloudEntitlements, AppError> {
        {
            let session = self.session.read().await;
            if let Some(entitlements) = session.entitlements.clone() {
                return Ok(entitlements);
            }
        }
        self.entitlements().await
    }

    pub async fn devices(&self) -> Result<Vec<CloudDevice>, AppError> {
        let response = self
            .authorized_json(Method::GET, "/api/v1/me/devices", None)
            .await?;
        parse_json(response).await
    }

    pub async fn remove_device(&self, id: Uuid) -> Result<(), AppError> {
        self.authorized_json(Method::DELETE, &format!("/api/v1/me/devices/{id}"), None)
            .await?;
        Ok(())
    }

    pub async fn ai_usage(&self) -> Result<CloudAiUsage, AppError> {
        let response = self
            .authorized_json(Method::GET, "/api/v1/ai/usage", None)
            .await?;
        parse_json(response).await
    }

    async fn refresh_me(&self) -> Result<(), AppError> {
        let response = self
            .authorized_json(Method::GET, "/api/v1/me", None)
            .await?;
        let me: CloudMeResponse = parse_json(response).await?;
        let mut session = self.session.write().await;
        session.user = Some(me.user);
        Ok(())
    }

    async fn refresh_entitlements(&self) -> Result<(), AppError> {
        let response = self
            .authorized_json(Method::GET, "/api/v1/me/entitlements", None)
            .await?;
        let entitlements: CloudEntitlements = parse_json(response).await?;
        let mut session = self.session.write().await;
        session.entitlements = Some(entitlements);
        Ok(())
    }

    async fn public_json(&self, path: &str, body: &Value) -> Result<Response, AppError> {
        let response = self
            .http
            .post(format!("{}{}", self.base_url(), path))
            .json(body)
            .send()
            .await
            .map_err(network_error)?;
        if !response.status().is_success() {
            return Err(self.map_response(response).await);
        }
        Ok(response)
    }

    pub async fn authorized_json(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Response, AppError> {
        let mut refreshed = false;
        loop {
            let token = self.access_token().await?;
            let mut request = self
                .http
                .request(method.clone(), format!("{}{}", self.base_url(), path))
                .bearer_auth(token);
            if let Some(body) = body {
                request = request.json(body);
            }
            let response = request.send().await.map_err(network_error)?;
            if response.status() == StatusCode::UNAUTHORIZED && !refreshed {
                refreshed = true;
                self.force_refresh().await?;
                continue;
            }
            if !response.status().is_success() {
                return Err(self.map_response(response).await);
            }
            return Ok(response);
        }
    }

    async fn map_response(&self, response: Response) -> AppError {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        let envelope = serde_json::from_str::<CloudErrorEnvelope>(&text).ok();
        let (code, message) = envelope
            .as_ref()
            .map(|value| (value.error.code.as_str(), value.error.message.as_str()))
            .unwrap_or(("unknown_error", text.as_str()));
        tracing::debug!(%status, code, "verruckt cloud request failed");
        match code {
            "invalid_credentials" | "invalid_email" | "invalid_password" => {
                AppError::CloudAuth("invalid email or password".into())
            }
            "access_token_expired"
            | "invalid_access_token"
            | "missing_access_token"
            | "session_revoked"
            | "refresh_token_expired"
            | "invalid_refresh_token"
            | "refresh_token_reuse_detected" => AppError::CloudAuth(message.to_owned()),
            "entitlement_required" => AppError::CloudEntitlementRequired,
            "ai_quota_exceeded" => AppError::CloudQuotaExceeded,
            "storage_quota_exceeded" => AppError::CloudStorageQuotaExceeded,
            "sync_conflict" => AppError::CloudSyncConflict(message.to_owned()),
            "device_limit_reached" => AppError::CloudEntitlementRequired,
            "email_already_registered" => {
                AppError::InvalidInput("email is already registered".into())
            }
            "object_storage_unavailable"
            | "object_storage_not_configured"
            | "upstream_unavailable"
            | "upstream_timeout"
            | "upstream_rate_limited"
            | "upstream_error"
            | "upstream_auth_failed"
            | "upstream_rejected_request"
            | "invalid_upstream_response"
            | "internal_error" => AppError::CloudUnavailable(format!("{code}: {message}")),
            _ => AppError::CloudUnavailable(format!("{code}: {message}")),
        }
    }

    async fn store_auth(&self, auth: &CloudAuthResponse) -> Result<(), AppError> {
        self.secrets
            .set_secret(REFRESH_TOKEN_REF, &auth.refresh_token)
            .await?;
        let expires_at = utc_now() + chrono::Duration::seconds(auth.expires_in.max(0));
        if let Some(device) = &auth.device {
            self.set_setting_string(DEVICE_ID_KEY, &device.id.to_string())
                .await?;
            self.set_setting_string(DEVICE_NAME_KEY, &device.name)
                .await?;
            self.set_setting_string(DEVICE_PLATFORM_KEY, &device.platform)
                .await?;
        }
        {
            let mut session = self.session.write().await;
            session.access_token = Some(auth.access_token.clone());
            session.access_expires_at = Some(expires_at);
            session.user = Some(auth.user.clone());
            session.device = auth.device.clone();
            session.entitlements = None;
        }
        self.emit(
            "cloud_session_changed",
            serde_json::json!({
                "loggedIn": true,
                "email": auth.user.email,
            }),
        );
        Ok(())
    }

    async fn clear_session(&self) {
        let _ = self.secrets.delete_secret(REFRESH_TOKEN_REF).await;
        {
            let mut session = self.session.write().await;
            *session = CloudSession::default();
        }
        self.emit("cloud_signed_out", serde_json::json!({}));
    }

    pub async fn device_id(&self) -> Result<Option<Uuid>, AppError> {
        Ok(self
            .setting_string(DEVICE_ID_KEY)
            .await?
            .and_then(|value| Uuid::parse_str(&value).ok()))
    }

    async fn device_input(&self) -> Result<CloudDeviceInput, AppError> {
        let name = self
            .setting_string(DEVICE_NAME_KEY)
            .await?
            .unwrap_or_else(default_device_name);
        let platform = self
            .setting_string(DEVICE_PLATFORM_KEY)
            .await?
            .unwrap_or_else(default_device_platform);
        Ok(CloudDeviceInput {
            name,
            platform,
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
        })
    }

    async fn setting_string(&self, key: &str) -> Result<Option<String>, AppError> {
        let value: Option<String> =
            sqlx::query_scalar("SELECT value_json FROM app_settings WHERE key = ?")
                .bind(key)
                .fetch_optional(&self.pool)
                .await?;
        Ok(value.and_then(|value| serde_json::from_str::<String>(&value).ok()))
    }

    async fn set_setting_string(&self, key: &str, value: &str) -> Result<(), AppError> {
        let json =
            serde_json::to_string(value).map_err(|error| AppError::Internal(error.to_string()))?;
        sqlx::query(
            "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?, ?, ?) \
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(json)
        .bind(utc_now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    fn emit(&self, event: &str, payload: Value) {
        let _ = self.events.send(CloudEvent {
            event: event.to_owned(),
            payload,
        });
    }
}

fn validate_credentials(email: &str, password: &str) -> Result<(), AppError> {
    if !email.contains('@') || email.trim().len() < 3 {
        return Err(AppError::InvalidInput("a valid email is required".into()));
    }
    if password.chars().count() < 8 {
        return Err(AppError::InvalidInput(
            "password must be at least 8 characters".into(),
        ));
    }
    Ok(())
}

fn network_error(error: reqwest::Error) -> AppError {
    AppError::CloudUnavailable(error.to_string())
}

async fn parse_json<T: DeserializeOwned>(response: Response) -> Result<T, AppError> {
    response
        .json()
        .await
        .map_err(|error| AppError::CloudUnavailable(error.to_string()))
}

pub fn default_cloud_base_url() -> String {
    std::env::var("VERRUCKT_CLOUD_URL")
        .ok()
        .and_then(|value| normalize_cloud_base_url(&value).ok())
        .unwrap_or_else(|| "http://127.0.0.1:8080".to_owned())
}

pub fn normalize_cloud_base_url(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput("cloud base URL is required".into()));
    }
    let parsed = url::Url::parse(trimmed)
        .map_err(|error| AppError::InvalidInput(format!("invalid cloud base URL: {error}")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::InvalidInput(
            "cloud base URL must use http or https".into(),
        ));
    }
    let mut normalized = trimmed.to_owned();
    if parsed.path().trim_end_matches('/') == "/v1" {
        normalized = normalized
            .trim_end_matches('/')
            .trim_end_matches("/v1")
            .trim_end_matches('/')
            .to_owned();
    }
    Ok(normalized)
}

fn default_device_platform() -> String {
    match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "macos",
        "linux" => "linux",
        "android" => "android",
        "ios" => "ios",
        other => other,
    }
    .to_owned()
}

fn default_device_name() -> String {
    if let Ok(name) = std::env::var("COMPUTERNAME") {
        if !name.trim().is_empty() {
            return name;
        }
    }
    if let Ok(name) = std::env::var("HOSTNAME") {
        if !name.trim().is_empty() {
            return name;
        }
    }
    match std::env::consts::OS {
        "windows" => "Windows PC",
        "macos" => "macOS Mac",
        "linux" => "Linux PC",
        "android" => "Android",
        "ios" => "iPhone",
        _ => "Desktop",
    }
    .to_owned()
}
