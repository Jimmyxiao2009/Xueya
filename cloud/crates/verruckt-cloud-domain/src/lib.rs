pub mod entitlements;
pub mod password;
pub mod sync;
pub mod token;

pub const PLAN_FREE: &str = "free";
pub const PLAN_PRO: &str = "pro";

pub const FEATURE_CLOUD_SYNC: &str = "cloud_sync";
pub const FEATURE_OFFICIAL_AI: &str = "official_ai";
pub const LIMIT_MAX_DEVICES: &str = "max_devices";
pub const LIMIT_MONTHLY_AI_CREDITS: &str = "monthly_ai_credits";
pub const LIMIT_MAX_CLOUD_STORAGE_BYTES: &str = "max_cloud_storage_bytes";

pub const COUNTER_AI_CREDITS: &str = "ai_credits";
pub const COUNTER_STORAGE_BYTES: &str = "storage_bytes";
