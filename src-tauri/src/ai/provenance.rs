//! Labels describe the selected provider and actual grounding, never UI intent.
use crate::{domain::ProviderConfig, error::AppError};
use super::registry::{is_mock_config, CLOUD_PROVIDER_ID};

pub fn require_real(config: &ProviderConfig) -> Result<(), AppError> {
    if is_mock_config(config) {
        Err(AppError::InvalidInput("Offline deterministic provider; real AI unavailable".into()))
    } else { Ok(()) }
}

pub fn generation_label(config: &ProviderConfig, grounded: bool) -> String {
    if is_mock_config(config) { return "mock".into(); }
    format!("{}_{}", if config.id == CLOUD_PROVIDER_ID { "official" } else { "byok" },
        if grounded { "grounded" } else { "general" })
}
