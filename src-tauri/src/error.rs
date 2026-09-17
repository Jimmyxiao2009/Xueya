use serde::ser::{SerializeStruct, Serializer};
use serde::Serialize;

/// Stable error codes understood by the frontend.
pub const DATABASE_ERROR: &str = "DATABASE_ERROR";
pub const IO_ERROR: &str = "IO_ERROR";
pub const INVALID_INPUT: &str = "INVALID_INPUT";
pub const NOT_FOUND: &str = "NOT_FOUND";
pub const PROVIDER_ERROR: &str = "PROVIDER_ERROR";
pub const PROVIDER_AUTH_ERROR: &str = "PROVIDER_AUTH_ERROR";
pub const NETWORK_ERROR: &str = "NETWORK_ERROR";
pub const SEARCH_ERROR: &str = "SEARCH_ERROR";
pub const PARSER_ERROR: &str = "PARSER_ERROR";
pub const EMBEDDING_ERROR: &str = "EMBEDDING_ERROR";
pub const AI_RESPONSE_INVALID: &str = "AI_RESPONSE_INVALID";
pub const JOB_CANCELLED: &str = "JOB_CANCELLED";
pub const UNSUPPORTED_FORMAT: &str = "UNSUPPORTED_FORMAT";
pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";

#[derive(Debug, Clone, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("provider error: {0}")]
    Provider(String),
    #[error("provider authentication error")]
    ProviderAuth,
    #[error("network error: {0}")]
    Network(String),
    #[error("search error: {0}")]
    Search(String),
    #[error("parser error: {0}")]
    Parser(String),
    #[error("embedding error: {0}")]
    Embedding(String),
    #[error("invalid AI response: {0}")]
    AiResponseInvalid(String),
    #[error("job cancelled")]
    JobCancelled,
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Database(_) => DATABASE_ERROR,
            Self::Io(_) => IO_ERROR,
            Self::InvalidInput(_) => INVALID_INPUT,
            Self::NotFound(_) => NOT_FOUND,
            Self::Provider(_) => PROVIDER_ERROR,
            Self::ProviderAuth => PROVIDER_AUTH_ERROR,
            Self::Network(_) => NETWORK_ERROR,
            Self::Search(_) => SEARCH_ERROR,
            Self::Parser(_) => PARSER_ERROR,
            Self::Embedding(_) => EMBEDDING_ERROR,
            Self::AiResponseInvalid(_) => AI_RESPONSE_INVALID,
            Self::JobCancelled => JOB_CANCELLED,
            Self::UnsupportedFormat(_) => UNSUPPORTED_FORMAT,
            Self::Internal(_) => INTERNAL_ERROR,
        }
    }

    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::Provider(_)
                | Self::Network(_)
                | Self::Search(_)
                | Self::Embedding(_)
                | Self::Database(_)
        )
    }

    /// Message safe for IPC. Raw errors are kept in `details` only when they
    /// are unlikely to contain credentials. Auth errors intentionally say
    /// nothing beyond the stable code.
    pub fn public_message(&self) -> &'static str {
        match self {
            Self::Database(_) => "Database operation failed.",
            Self::Io(_) => "File operation failed.",
            Self::InvalidInput(_) => "The input is invalid.",
            Self::NotFound(_) => "The requested resource was not found.",
            Self::Provider(_) => "AI provider request failed.",
            Self::ProviderAuth => "AI provider authentication failed.",
            Self::Network(_) => "Network request failed.",
            Self::Search(_) => "Search provider request failed.",
            Self::Parser(_) => "Document parsing failed.",
            Self::Embedding(_) => "Embedding request failed.",
            Self::AiResponseInvalid(_) => "The AI returned an invalid structured response.",
            Self::JobCancelled => "The job was cancelled.",
            Self::UnsupportedFormat(_) => "The document format is not supported.",
            Self::Internal(_) => "An internal error occurred.",
        }
    }

    pub fn details(&self) -> Option<String> {
        match self {
            Self::InvalidInput(message)
            | Self::NotFound(message)
            | Self::UnsupportedFormat(message) => Some(redact(message)),
            Self::Database(message)
            | Self::Io(message)
            | Self::Provider(message)
            | Self::Network(message)
            | Self::Search(message)
            | Self::Parser(message)
            | Self::Embedding(message)
            | Self::AiResponseInvalid(message)
            | Self::Internal(message) => Some(redact(message)),
            Self::ProviderAuth | Self::JobCancelled => None,
        }
    }

    pub fn context(self, context: impl Into<String>) -> Self {
        let context = context.into();
        match self {
            Self::Database(message) => Self::Database(format!("{context}: {message}")),
            Self::Io(message) => Self::Io(format!("{context}: {message}")),
            Self::InvalidInput(message) => Self::InvalidInput(format!("{context}: {message}")),
            Self::NotFound(message) => Self::NotFound(format!("{context}: {message}")),
            Self::Provider(message) => Self::Provider(format!("{context}: {message}")),
            Self::ProviderAuth => Self::ProviderAuth,
            Self::Network(message) => Self::Network(format!("{context}: {message}")),
            Self::Search(message) => Self::Search(format!("{context}: {message}")),
            Self::Parser(message) => Self::Parser(format!("{context}: {message}")),
            Self::Embedding(message) => Self::Embedding(format!("{context}: {message}")),
            Self::AiResponseInvalid(message) => {
                Self::AiResponseInvalid(format!("{context}: {message}"))
            }
            Self::JobCancelled => Self::JobCancelled,
            Self::UnsupportedFormat(message) => {
                Self::UnsupportedFormat(format!("{context}: {message}"))
            }
            Self::Internal(message) => Self::Internal(format!("{context}: {message}")),
        }
    }
}

/// Avoid ever emitting bearer tokens / API keys through IPC or logs.
pub fn redact(value: &str) -> String {
    let mut redacted = value.to_owned();
    for marker in [
        "Bearer ",
        "sk-",
        "api_key=",
        "apikey=",
        "x-subscription-token",
    ] {
        while let Some(index) = redacted
            .to_ascii_lowercase()
            .find(&marker.to_ascii_lowercase())
        {
            let end = (index + 512).min(redacted.len());
            let replacement = format!("{}[REDACTED]", &redacted[..index]);
            redacted = format!("{replacement}{}", &redacted[end..]);
        }
    }
    redacted
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        Self::InvalidInput(error.to_string())
    }
}

impl From<uuid::Error> for AppError {
    fn from(error: uuid::Error) -> Self {
        Self::InvalidInput(error.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("AppError", 4)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", self.public_message())?;
        state.serialize_field("details", &self.details())?;
        state.serialize_field("retryable", &self.retryable())?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialization_has_stable_shape_and_redacts_secrets() {
        let error = AppError::ProviderAuth;
        let json = serde_json::to_value(&error).expect("serialize");
        assert_eq!(json["code"], PROVIDER_AUTH_ERROR);
        assert_eq!(json["retryable"], false);
        assert!(json["details"].is_null());

        let error = AppError::Provider("Authorization: Bearer sk-test-secret".into());
        let json = serde_json::to_value(&error).expect("serialize");
        let details = json["details"].as_str().expect("details");
        assert!(!details.contains("sk-test-secret"));
        assert!(details.contains("[REDACTED]"));
    }
}
