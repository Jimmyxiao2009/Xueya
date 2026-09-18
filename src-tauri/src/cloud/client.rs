use super::models::{
    CloudAccountSnapshot, CloudAccountState, CloudAiUsage, CloudAuthResponse, CloudDevice,
    CloudDeviceDto, CloudDeviceInput, CloudEntitlements, CloudEntitlementsDto, CloudErrorEnvelope,
    CloudLoginRequest, CloudLogoutRequest, CloudMeResponse, CloudRefreshRequest,
    CloudRegisterRequest, CloudSession, CloudSessionState,
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
    restore_lock: AsyncMutex<()>,
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
            restore_lock: AsyncMutex::new(()),
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
        if self.secrets.secret_exists(REFRESH_TOKEN_REF).await? {
            self.session.write().await.state = CloudSessionState::Restoring;
            self.emit("cloud_session_restoring", serde_json::json!({}));
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
        let response = match self
            .http
            .post(format!("{}/api/v1/auth/refresh", self.base_url()))
            .json(&body)
            .send()
            .await
        {
            Ok(response) => response,
            Err(error) => {
                let error = network_error(error);
                self.mark_degraded().await;
                return Err(error);
            }
        };

        if response.status() == StatusCode::UNAUTHORIZED {
            let error = self.map_response(response).await;
            self.clear_session().await;
            return Err(error);
        }
        if !response.status().is_success() {
            let error = self.map_response(response).await;
            self.mark_degraded().await;
            return Err(error);
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
        // Fast path: a fully restored, in-memory session.
        {
            let session = self.session.read().await;
            if session.state == CloudSessionState::Authenticated
                && session.user.is_some()
                && session.entitlements.is_some()
            {
                drop(session);
                return self.current_state().await;
            }
        }

        if !self.secrets.secret_exists(REFRESH_TOKEN_REF).await? {
            self.clear_session().await;
            return self.anonymous_state().await;
        }

        // Only one caller restores at a time. Other callers report
        // `Restoring` instead of briefly falling back to an anonymous state.
        let Ok(_guard) = self.restore_lock.try_lock() else {
            return self.state_from_snapshot(CloudSessionState::Restoring).await;
        };

        // Re-check after taking the lock: another restore may have completed
        // while this caller waited for the fast path.
        {
            let session = self.session.read().await;
            if session.state == CloudSessionState::Authenticated
                && session.user.is_some()
                && session.entitlements.is_some()
            {
                drop(session);
                return self.current_state().await;
            }
        }

        self.set_session_state(CloudSessionState::Restoring).await;
        match self.try_restore().await {
            Ok(()) => {
                self.set_session_state(CloudSessionState::Authenticated)
                    .await;
                self.current_state().await
            }
            Err(error) if is_auth_error(&error) => {
                // Explicit 401 / invalid refresh / reuse detection: the
                // credential is no longer trustworthy.
                self.clear_session().await;
                self.anonymous_state().await
            }
            Err(error) => {
                tracing::warn!(%error, "cloud session is degraded; local learning continues");
                self.set_session_state(CloudSessionState::DegradedOffline)
                    .await;
                self.state_from_snapshot(CloudSessionState::DegradedOffline)
                    .await
            }
        }
    }

    async fn try_restore(&self) -> Result<(), AppError> {
        self.access_token().await?;
        // `/me` and `/me/entitlements` are the authoritative snapshot; always
        // refresh both during restore so a successful login can never show a
        // partially populated account.
        self.refresh_me().await?;
        self.refresh_entitlements().await?;
        Ok(())
    }

    async fn set_session_state(&self, state: CloudSessionState) {
        {
            let mut session = self.session.write().await;
            session.state = state;
        }
        self.emit(
            "cloud_session_state_changed",
            serde_json::json!({ "state": state }),
        );
    }

    async fn mark_degraded(&self) {
        let should_mark = {
            let session = self.session.read().await;
            session.state != CloudSessionState::SignedOut
        };
        if should_mark {
            self.set_session_state(CloudSessionState::DegradedOffline)
                .await;
        }
    }

    async fn anonymous_state(&self) -> Result<CloudAccountState, AppError> {
        Ok(CloudAccountState {
            state: CloudSessionState::SignedOut,
            logged_in: false,
            email: None,
            user_id: None,
            plan: None,
            entitlements: None,
            device: None,
            sync_enabled: self.sync_enabled().await?,
            base_url: self.base_url(),
            last_successful_refresh_at: None,
        })
    }

    async fn current_state(&self) -> Result<CloudAccountState, AppError> {
        let session = self.session.read().await;
        let state = session.state;
        drop(session);
        self.current_state_with_state(state).await
    }

    async fn current_state_with_state(
        &self,
        state: CloudSessionState,
    ) -> Result<CloudAccountState, AppError> {
        let session = self.session.read().await;
        let entitlements = session
            .entitlements
            .as_ref()
            .map(CloudEntitlementsDto::from);
        Ok(CloudAccountState {
            state,
            logged_in: state == CloudSessionState::Authenticated && session.user.is_some(),
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
            last_successful_refresh_at: session.last_successful_refresh_at,
        })
    }

    /// Build a safe state from the persisted snapshot. This is the only path
    /// that can show an account while the cloud is unreachable.
    async fn state_from_snapshot(
        &self,
        state: CloudSessionState,
    ) -> Result<CloudAccountState, AppError> {
        let Some(snapshot) = self.load_snapshot().await? else {
            return self.current_state_with_state(state).await;
        };
        let entitlements = snapshot
            .entitlements
            .as_ref()
            .map(CloudEntitlementsDto::from);
        Ok(CloudAccountState {
            state,
            logged_in: state == CloudSessionState::Authenticated,
            email: Some(snapshot.email),
            user_id: Some(snapshot.user_id),
            plan: snapshot.plan.or_else(|| {
                snapshot
                    .entitlements
                    .as_ref()
                    .map(|value| value.plan.clone())
            }),
            entitlements,
            device: None,
            sync_enabled: self.sync_enabled().await?,
            base_url: self.base_url(),
            last_successful_refresh_at: Some(snapshot.last_successful_refresh_at),
        })
    }

    async fn load_snapshot(&self) -> Result<Option<CloudAccountSnapshot>, AppError> {
        let payload: Option<String> =
            sqlx::query_scalar("SELECT payload_json FROM cloud_account_snapshot WHERE id = 1")
                .fetch_optional(&self.pool)
                .await?;
        Ok(payload.and_then(|value| serde_json::from_str(&value).ok()))
    }

    async fn persist_snapshot(&self) -> Result<(), AppError> {
        let session = self.session.read().await;
        let Some(user) = session.user.as_ref() else {
            return Ok(());
        };
        let snapshot = CloudAccountSnapshot {
            user_id: user.id,
            email: user.email.clone(),
            display_name: user.display_name.clone(),
            plan: session
                .entitlements
                .as_ref()
                .map(|value| value.plan.clone()),
            entitlements: session.entitlements.clone(),
            usage: session.usage.clone(),
            last_successful_refresh_at: session.last_successful_refresh_at.unwrap_or_else(utc_now),
            saved_at: utc_now(),
        };
        drop(session);
        let payload = serde_json::to_string(&snapshot)
            .map_err(|error| AppError::Internal(error.to_string()))?;
        sqlx::query(
            "INSERT INTO cloud_account_snapshot (id, payload_json, updated_at) VALUES (1, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET payload_json = excluded.payload_json, updated_at = excluded.updated_at",
        )
        .bind(payload)
        .bind(utc_now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn clear_snapshot(&self) {
        let _ = sqlx::query("DELETE FROM cloud_account_snapshot WHERE id = 1")
            .execute(&self.pool)
            .await;
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
        // DegradedOffline still needs the last known plan for UI gating. The
        // gateway remains the source of truth, so this snapshot can never grant
        // a real server-side entitlement.
        if let Some(snapshot) = self.load_snapshot().await? {
            if let Some(entitlements) = snapshot.entitlements {
                let mut session = self.session.write().await;
                session.entitlements = Some(entitlements.clone());
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
        let usage: CloudAiUsage = parse_json(response).await?;
        {
            let mut session = self.session.write().await;
            session.usage = Some(usage.clone());
            session.last_successful_refresh_at = Some(utc_now());
        }
        self.persist_snapshot().await?;
        Ok(usage)
    }

    async fn refresh_me(&self) -> Result<(), AppError> {
        let response = self
            .authorized_json(Method::GET, "/api/v1/me", None)
            .await?;
        let me: CloudMeResponse = parse_json(response).await?;
        {
            let mut session = self.session.write().await;
            session.user = Some(me.user);
            session.last_successful_refresh_at = Some(utc_now());
        }
        self.persist_snapshot().await?;
        Ok(())
    }

    async fn refresh_entitlements(&self) -> Result<(), AppError> {
        let response = self
            .authorized_json(Method::GET, "/api/v1/me/entitlements", None)
            .await?;
        let entitlements: CloudEntitlements = parse_json(response).await?;
        {
            let mut session = self.session.write().await;
            session.entitlements = Some(entitlements);
            session.last_successful_refresh_at = Some(utc_now());
        }
        self.persist_snapshot().await?;
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
        let (mut previous_usage, mut previous_entitlements) = {
            let session = self.session.read().await;
            (session.usage.clone(), session.entitlements.clone())
        };
        // A refresh response does not carry entitlements. Keep the last known
        // non-secret snapshot so a later transient failure can still show the
        // account and plan instead of degrading to anonymous.
        if previous_usage.is_none() || previous_entitlements.is_none() {
            if let Some(snapshot) = self.load_snapshot().await? {
                if previous_usage.is_none() {
                    previous_usage = snapshot.usage;
                }
                if previous_entitlements.is_none() {
                    previous_entitlements = snapshot.entitlements;
                }
            }
        }
        {
            let mut session = self.session.write().await;
            session.state = CloudSessionState::Authenticated;
            session.access_token = Some(auth.access_token.clone());
            session.access_expires_at = Some(expires_at);
            session.user = Some(auth.user.clone());
            session.device = auth.device.clone();
            session.entitlements = previous_entitlements;
            session.usage = previous_usage;
            session.last_successful_refresh_at = Some(utc_now());
        }
        self.persist_snapshot().await?;
        self.emit(
            "cloud_session_changed",
            serde_json::json!({
                "state": "authenticated",
                "loggedIn": true,
                "email": auth.user.email,
            }),
        );
        Ok(())
    }

    async fn clear_session(&self) {
        let _ = self.secrets.delete_secret(REFRESH_TOKEN_REF).await;
        self.clear_snapshot().await;
        {
            let mut session = self.session.write().await;
            *session = CloudSession::default();
        }
        self.emit("cloud_signed_out", serde_json::json!({}));
        self.emit(
            "cloud_session_state_changed",
            serde_json::json!({ "state": "signedOut" }),
        );
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

/// Only credential/session failures may delete the refresh token. Transient
/// network or gateway failures must keep it so offline learning can continue.
fn is_auth_error(error: &AppError) -> bool {
    matches!(
        error,
        AppError::CloudAuth(_) | AppError::CloudSessionExpired
    )
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
