-- Non-secret account snapshot used by the DegradedOffline UI.
--
-- The snapshot deliberately excludes access tokens, refresh tokens, API keys,
-- and any other secret. It lets the app show the last known email, plan,
-- entitlements, and usage while the network or cloud gateway is unavailable.
CREATE TABLE IF NOT EXISTS cloud_account_snapshot (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    payload_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
