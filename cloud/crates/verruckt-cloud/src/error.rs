use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use verruckt_cloud_protocol::error::{ApiErrorBody, ApiErrorEnvelope};
use verruckt_cloud_storage::StorageError;

use crate::request_id::current_request_id;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl ApiError {
    pub fn new(
        status: StatusCode,
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
            retryable,
            source: None,
        }
    }

    pub fn with_source(mut self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    pub fn bad_request(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message, false)
    }

    pub fn unauthorized(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, code, message, false)
    }

    pub fn forbidden(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, code, message, false)
    }

    pub fn not_found(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, code, message, false)
    }

    pub fn conflict(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, code, message, false)
    }

    pub fn payload_too_large(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::PAYLOAD_TOO_LARGE, code, message, false)
    }

    pub fn unprocessable(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, code, message, false)
    }

    pub fn too_many_requests(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::TOO_MANY_REQUESTS, code, message, true)
    }

    pub fn payment_required(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::PAYMENT_REQUIRED, code, message, false)
    }

    pub fn service_unavailable(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, code, message, true)
    }

    pub fn bad_gateway(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_GATEWAY, code, message, true)
    }

    pub fn gateway_timeout(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::GATEWAY_TIMEOUT, code, message, true)
    }

    pub fn internal(source: impl std::error::Error + Send + Sync + 'static) -> Self {
        tracing::error!(error = %source, "internal server error");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "the server encountered an internal error",
            true,
        )
        .with_source(source)
    }

    pub fn internal_message(message: impl Into<String>) -> Self {
        let message = message.into();
        tracing::error!(error = %message, "internal server error");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "the server encountered an internal error",
            true,
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if self.status.is_server_error() {
            tracing::error!(
                status = self.status.as_u16(),
                code = %self.code,
                "request failed"
            );
        } else {
            tracing::debug!(
                status = self.status.as_u16(),
                code = %self.code,
                "request rejected"
            );
        }

        let body = ApiErrorEnvelope {
            error: ApiErrorBody {
                code: self.code,
                message: self.message,
                retryable: self.retryable,
                request_id: current_request_id(),
            },
        };

        (self.status, Json(body)).into_response()
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        Self::internal(error)
    }
}

impl From<StorageError> for ApiError {
    fn from(error: StorageError) -> Self {
        tracing::error!(error = %error, "object storage operation failed");
        Self::service_unavailable(
            "object_storage_unavailable",
            "object storage is temporarily unavailable",
        )
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(error: serde_json::Error) -> Self {
        Self::bad_request("invalid_json", format!("invalid JSON: {error}"))
    }
}
