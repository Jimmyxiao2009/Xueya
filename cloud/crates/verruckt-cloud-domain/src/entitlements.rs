use crate::{
    FEATURE_CLOUD_SYNC, FEATURE_OFFICIAL_AI, LIMIT_MAX_CLOUD_STORAGE_BYTES, LIMIT_MAX_DEVICES,
    LIMIT_MONTHLY_AI_CREDITS, PLAN_FREE,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanEntitlement {
    pub key: String,
    pub enabled: bool,
    pub limit_value: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntitlementSet {
    pub plan: String,
    pub features: BTreeMap<String, bool>,
    pub limits: BTreeMap<String, i64>,
}

impl EntitlementSet {
    pub fn from_rows(
        plan: impl Into<String>,
        rows: impl IntoIterator<Item = PlanEntitlement>,
    ) -> Self {
        let plan = plan.into();
        let mut features = BTreeMap::new();
        let mut limits = BTreeMap::new();

        for row in rows {
            if let Some(limit) = row.limit_value {
                limits.insert(row.key.clone(), limit);
            }
            features.insert(row.key, row.enabled);
        }

        Self {
            plan,
            features,
            limits,
        }
    }

    pub fn has_feature(&self, key: &str) -> bool {
        self.features.get(key).copied().unwrap_or(false)
    }

    pub fn limit(&self, key: &str) -> Option<i64> {
        self.limits.get(key).copied()
    }

    pub fn max_devices(&self) -> i64 {
        self.limit(LIMIT_MAX_DEVICES).unwrap_or(1)
    }

    pub fn monthly_ai_credits(&self) -> i64 {
        self.limit(LIMIT_MONTHLY_AI_CREDITS).unwrap_or(0)
    }

    pub fn max_cloud_storage_bytes(&self) -> i64 {
        self.limit(LIMIT_MAX_CLOUD_STORAGE_BYTES).unwrap_or(0)
    }

    pub fn cloud_sync_enabled(&self) -> bool {
        self.has_feature(FEATURE_CLOUD_SYNC)
    }

    pub fn official_ai_enabled(&self) -> bool {
        self.has_feature(FEATURE_OFFICIAL_AI)
    }
}

impl Default for EntitlementSet {
    fn default() -> Self {
        Self::from_rows(
            PLAN_FREE,
            [
                PlanEntitlement {
                    key: FEATURE_CLOUD_SYNC.into(),
                    enabled: false,
                    limit_value: None,
                },
                PlanEntitlement {
                    key: FEATURE_OFFICIAL_AI.into(),
                    enabled: false,
                    limit_value: None,
                },
                PlanEntitlement {
                    key: LIMIT_MAX_DEVICES.into(),
                    enabled: true,
                    limit_value: Some(1),
                },
                PlanEntitlement {
                    key: LIMIT_MONTHLY_AI_CREDITS.into(),
                    enabled: true,
                    limit_value: Some(0),
                },
                PlanEntitlement {
                    key: LIMIT_MAX_CLOUD_STORAGE_BYTES.into(),
                    enabled: true,
                    limit_value: Some(0),
                },
            ],
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageSnapshot {
    pub devices_used: i64,
    pub monthly_ai_credits_used: i64,
    pub cloud_storage_bytes_used: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_plan_features_and_limits() {
        let set = EntitlementSet::from_rows(
            "pro",
            [
                PlanEntitlement {
                    key: FEATURE_CLOUD_SYNC.into(),
                    enabled: true,
                    limit_value: None,
                },
                PlanEntitlement {
                    key: FEATURE_OFFICIAL_AI.into(),
                    enabled: true,
                    limit_value: None,
                },
                PlanEntitlement {
                    key: LIMIT_MAX_DEVICES.into(),
                    enabled: true,
                    limit_value: Some(5),
                },
                PlanEntitlement {
                    key: LIMIT_MONTHLY_AI_CREDITS.into(),
                    enabled: true,
                    limit_value: Some(100_000),
                },
                PlanEntitlement {
                    key: LIMIT_MAX_CLOUD_STORAGE_BYTES.into(),
                    enabled: true,
                    limit_value: Some(5 * 1024 * 1024 * 1024),
                },
            ],
        );
        assert!(set.cloud_sync_enabled());
        assert!(set.official_ai_enabled());
        assert_eq!(set.max_devices(), 5);
        assert_eq!(set.monthly_ai_credits(), 100_000);
        assert_eq!(set.max_cloud_storage_bytes(), 5 * 1024 * 1024 * 1024);
    }

    #[test]
    fn defaults_are_deny_by_default() {
        let set = EntitlementSet::default();
        assert!(!set.cloud_sync_enabled());
        assert!(!set.official_ai_enabled());
        assert_eq!(set.max_devices(), 1);
        assert_eq!(set.monthly_ai_credits(), 0);
    }
}
