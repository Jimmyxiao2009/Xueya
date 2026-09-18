use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudDeviceInput {
    pub name: String,
    pub platform: String,
    pub app_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudRegisterRequest {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub device: Option<CloudDeviceInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudLoginRequest {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub device: Option<CloudDeviceInput>,
    #[serde(default)]
    pub device_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudRefreshRequest {
    pub refresh_token: String,
    #[serde(default)]
    pub device: Option<CloudDeviceInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CloudLogoutRequest {
    #[serde(default)]
    pub refresh_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudUser {
    pub id: Uuid,
    pub email: String,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudDevice {
    pub id: Uuid,
    pub name: String,
    pub platform: String,
    pub app_version: String,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudAuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub user: CloudUser,
    #[serde(default)]
    pub device: Option<CloudDevice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudMeResponse {
    pub user: CloudUser,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudEntitlementsUsage {
    pub devices_used: i64,
    pub monthly_ai_credits_used: i64,
    pub cloud_storage_bytes_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudEntitlements {
    pub plan: String,
    pub features: BTreeMap<String, bool>,
    pub limits: BTreeMap<String, i64>,
    pub usage: CloudEntitlementsUsage,
}

impl CloudEntitlements {
    pub fn feature(&self, name: &str) -> bool {
        self.features.get(name).copied().unwrap_or(false)
    }

    pub fn limit(&self, name: &str) -> i64 {
        self.limits.get(name).copied().unwrap_or(0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudAiUsageEvent {
    pub id: Uuid,
    pub operation: String,
    pub logical_model: String,
    pub upstream_model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub estimated_cost_units: i64,
    pub status: String,
    pub request_id: Uuid,
    pub latency_ms: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudAiUsage {
    pub period_start: DateTime<Utc>,
    pub credits_used: i64,
    pub credits_limit: i64,
    pub credits_remaining: i64,
    #[serde(default)]
    pub events: Vec<CloudAiUsageEvent>,
}

/// Non-secret account state safe to send to the frontend. The refresh token is
/// never part of this shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudAccountState {
    pub logged_in: bool,
    pub email: Option<String>,
    pub user_id: Option<Uuid>,
    pub plan: Option<String>,
    pub entitlements: Option<CloudEntitlementsDto>,
    pub device: Option<CloudDeviceDto>,
    pub sync_enabled: bool,
    pub base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudEntitlementsDto {
    pub cloud_sync: bool,
    pub official_ai: bool,
    pub max_devices: i64,
    pub monthly_ai_credits: i64,
    pub storage_bytes: i64,
    pub devices_used: i64,
    pub monthly_ai_credits_used: i64,
    pub cloud_storage_bytes_used: i64,
    pub plan: String,
}

impl From<&CloudEntitlements> for CloudEntitlementsDto {
    fn from(value: &CloudEntitlements) -> Self {
        Self {
            cloud_sync: value.feature("cloud_sync"),
            official_ai: value.feature("official_ai"),
            max_devices: value.limit("max_devices"),
            monthly_ai_credits: value.limit("monthly_ai_credits"),
            storage_bytes: value.limit("max_cloud_storage_bytes"),
            devices_used: value.usage.devices_used,
            monthly_ai_credits_used: value.usage.monthly_ai_credits_used,
            cloud_storage_bytes_used: value.usage.cloud_storage_bytes_used,
            plan: value.plan.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudDeviceDto {
    pub id: Uuid,
    pub name: String,
    pub platform: String,
    pub app_version: String,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

impl From<CloudDevice> for CloudDeviceDto {
    fn from(value: CloudDevice) -> Self {
        Self {
            id: value.id,
            name: value.name,
            platform: value.platform,
            app_version: value.app_version,
            created_at: value.created_at,
            last_seen_at: value.last_seen_at,
        }
    }
}

/// Internal in-memory session. The access token lives only here; the refresh
/// token lives in the OS credential store.
#[derive(Debug, Clone, Default)]
pub struct CloudSession {
    pub access_token: Option<String>,
    pub access_expires_at: Option<DateTime<Utc>>,
    pub user: Option<CloudUser>,
    pub device: Option<CloudDevice>,
    pub entitlements: Option<CloudEntitlements>,
}

impl CloudSession {
    pub fn has_valid_access_token(&self) -> bool {
        let Some(token) = self.access_token.as_deref() else {
            return false;
        };
        if token.trim().is_empty() {
            return false;
        }
        match self.access_expires_at {
            Some(expires_at) => expires_at > Utc::now() + chrono::Duration::seconds(30),
            None => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudMeDto {
    pub id: Uuid,
    pub email: String,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<CloudUser> for CloudMeDto {
    fn from(value: CloudUser) -> Self {
        Self {
            id: value.id,
            email: value.email,
            display_name: value.display_name,
            created_at: value.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudUsageDto {
    pub period_start: DateTime<Utc>,
    pub credits_used: i64,
    pub credits_limit: i64,
    pub credits_remaining: i64,
}

impl From<&CloudAiUsage> for CloudUsageDto {
    fn from(value: &CloudAiUsage) -> Self {
        Self {
            period_start: value.period_start,
            credits_used: value.credits_used,
            credits_limit: value.credits_limit,
            credits_remaining: value.credits_remaining,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudErrorBody {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub retryable: bool,
    #[serde(default)]
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudErrorEnvelope {
    pub error: CloudErrorBody,
}
