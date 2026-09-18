use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Which OpenAI-compatible wire protocol to use.
///
/// `Auto` asks the capability probe to pick. The other two pin the protocol so
/// a provider that exposes both endpoints can still be tested against the one
/// the user knows works.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, Default)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum ProviderProtocol {
    #[default]
    Auto,
    Responses,
    ChatCompletions,
}

impl ProviderProtocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Responses => "responses",
            Self::ChatCompletions => "chat_completions",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "responses" | "response" => Self::Responses,
            "chat_completions" | "chat-completions" | "chat" => Self::ChatCompletions,
            _ => Self::Auto,
        }
    }
}

/// Which AI source the user chose in Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, Default)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum AiMode {
    /// Deterministic local mock. Always available, including offline demos.
    #[default]
    Offline,
    /// The user's own OpenAI-compatible endpoint / API key.
    Byok,
    /// Verruckt Cloud official AI gateway.
    Cloud,
}

impl AiMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Offline => "offline",
            Self::Byok => "byok",
            Self::Cloud => "cloud",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "cloud" | "verruckt" | "official" => Self::Cloud,
            "byok" | "custom" | "custom_api" => Self::Byok,
            _ => Self::Offline,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProviderConfig {
    pub id: Uuid,
    pub display_name: String,
    pub base_url: String,
    pub chat_model: String,
    pub embedding_model: String,
    pub api_key_ref: Option<String>,
    pub protocol: ProviderProtocol,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
