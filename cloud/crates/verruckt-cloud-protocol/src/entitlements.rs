use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitlementsUsage {
    pub devices_used: i64,
    pub monthly_ai_credits_used: i64,
    pub cloud_storage_bytes_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitlementsResponse {
    pub plan: String,
    pub features: BTreeMap<String, bool>,
    pub limits: BTreeMap<String, i64>,
    pub usage: EntitlementsUsage,
}
