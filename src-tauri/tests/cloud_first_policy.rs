//! Account-aware Cloud-first policy tests.
//!
//! These tests do not call a real cloud. They exercise the same reconciliation
//! functions used by `app_bootstrap`, login/register, and logout while keeping
//! local Core state untouched.

use std::sync::Arc;

use uuid::Uuid;
use verruckt_lib::ai::registry::ProviderUpsert;
use verruckt_lib::cloud::models::{CloudAccountState, CloudEntitlementsDto, CloudSessionState};
use verruckt_lib::domain::ProviderProtocol;
use verruckt_lib::ipc::dto::SettingsUpdateInput;
use verruckt_lib::secrets::keyring_store::InMemorySecretStore;
use verruckt_lib::services::settings_service;
use verruckt_lib::state::{AppPaths, AppState};

async fn fresh_state() -> (AppState, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let state = AppState::new_with_secrets(
        AppPaths::new(dir.path()),
        Arc::new(InMemorySecretStore::new()),
    )
    .await
    .expect("app state");
    (state, dir)
}

fn entitlements(official_ai: bool, cloud_sync: bool) -> CloudEntitlementsDto {
    CloudEntitlementsDto {
        cloud_sync,
        official_ai,
        max_devices: 5,
        monthly_ai_credits: 1000,
        storage_bytes: 5 * 1024 * 1024 * 1024,
        devices_used: 1,
        monthly_ai_credits_used: 0,
        cloud_storage_bytes_used: 0,
        plan: "pro".to_owned(),
    }
}

fn signed_in_account(official_ai: bool, cloud_sync: bool) -> CloudAccountState {
    CloudAccountState {
        state: CloudSessionState::Authenticated,
        logged_in: true,
        email: Some("policy@example.com".to_owned()),
        user_id: Some(Uuid::new_v4()),
        plan: Some("pro".to_owned()),
        entitlements: Some(entitlements(official_ai, cloud_sync)),
        device: None,
        sync_enabled: false,
        base_url: "https://cloud.example".to_owned(),
        last_successful_refresh_at: None,
    }
}

fn signed_out_account() -> CloudAccountState {
    CloudAccountState {
        state: CloudSessionState::SignedOut,
        logged_in: false,
        email: None,
        user_id: None,
        plan: None,
        entitlements: None,
        device: None,
        sync_enabled: false,
        base_url: "https://cloud.example".to_owned(),
        last_successful_refresh_at: None,
    }
}

async fn add_valid_byok(state: &AppState) {
    state
        .providers
        .upsert(ProviderUpsert {
            id: None,
            display_name: "Policy test BYOK".to_owned(),
            base_url: "https://byok.example/v1".to_owned(),
            chat_model: "test-chat".to_owned(),
            embedding_model: "test-embedding".to_owned(),
            api_key: Some("test-key".to_owned()),
            protocol: ProviderProtocol::ChatCompletions,
            enabled: true,
        })
        .await
        .expect("add byok provider");
}

async fn select_explicit_mode(state: &AppState, mode: &str) {
    settings_service::update_settings(
        state,
        SettingsUpdateInput {
            search_provider: None,
            brave_api_key: None,
            tavily_api_key: None,
            serper_api_key: None,
            searxng_url: None,
            ai_mode: Some(mode.to_owned()),
        },
    )
    .await
    .expect("select explicit ai mode");
}

async fn ai_mode(state: &AppState) -> String {
    settings_service::get_settings(state)
        .await
        .expect("settings")
        .ai_mode
}

#[tokio::test]
async fn fresh_signed_out_without_byok_defaults_to_offline() {
    let (state, _dir) = fresh_state().await;
    let account = signed_out_account();

    settings_service::reconcile_provider_policy(&state, &account)
        .await
        .expect("reconcile");

    assert_eq!(ai_mode(&state).await, "offline");
}

#[tokio::test]
async fn signed_in_account_with_official_ai_defaults_to_cloud() {
    let (state, _dir) = fresh_state().await;
    let account = signed_in_account(true, true);

    settings_service::reconcile_provider_policy(&state, &account)
        .await
        .expect("reconcile");

    assert_eq!(ai_mode(&state).await, "cloud");
}

#[tokio::test]
async fn account_without_official_ai_does_not_default_to_cloud() {
    let (state, _dir) = fresh_state().await;
    let account = signed_in_account(false, true);

    settings_service::reconcile_provider_policy(&state, &account)
        .await
        .expect("reconcile");

    assert_eq!(ai_mode(&state).await, "offline");
}

#[tokio::test]
async fn account_without_official_ai_uses_valid_byok() {
    let (state, _dir) = fresh_state().await;
    add_valid_byok(&state).await;
    let account = signed_in_account(false, true);

    settings_service::reconcile_provider_policy(&state, &account)
        .await
        .expect("reconcile");

    assert_eq!(ai_mode(&state).await, "byok");
}

#[tokio::test]
async fn explicit_byok_survives_cloud_login() {
    let (state, _dir) = fresh_state().await;
    add_valid_byok(&state).await;
    select_explicit_mode(&state, "byok").await;
    let account = signed_in_account(true, true);

    settings_service::reconcile_provider_policy(&state, &account)
        .await
        .expect("reconcile");

    assert_eq!(ai_mode(&state).await, "byok");
}

#[tokio::test]
async fn explicit_offline_survives_cloud_login() {
    let (state, _dir) = fresh_state().await;
    select_explicit_mode(&state, "offline").await;
    let account = signed_in_account(true, true);

    settings_service::reconcile_provider_policy(&state, &account)
        .await
        .expect("reconcile");

    assert_eq!(ai_mode(&state).await, "offline");
}

#[tokio::test]
async fn logout_from_auto_cloud_falls_back_to_offline_without_byok() {
    let (state, _dir) = fresh_state().await;
    let cloud = signed_in_account(true, true);
    settings_service::reconcile_provider_policy(&state, &cloud)
        .await
        .expect("cloud reconcile");
    assert_eq!(ai_mode(&state).await, "cloud");

    settings_service::reconcile_provider_policy(&state, &signed_out_account())
        .await
        .expect("logout reconcile");

    assert_eq!(ai_mode(&state).await, "offline");
}

#[tokio::test]
async fn logout_from_auto_cloud_falls_back_to_valid_byok() {
    let (state, _dir) = fresh_state().await;
    add_valid_byok(&state).await;
    let cloud = signed_in_account(true, true);
    settings_service::reconcile_provider_policy(&state, &cloud)
        .await
        .expect("cloud reconcile");
    assert_eq!(ai_mode(&state).await, "cloud");

    settings_service::reconcile_provider_policy(&state, &signed_out_account())
        .await
        .expect("logout reconcile");

    assert_eq!(ai_mode(&state).await, "byok");
}

#[tokio::test]
async fn zero_credit_cloud_account_does_not_default_to_cloud() {
    let (state, _dir) = fresh_state().await;
    let mut account = signed_in_account(true, true);
    account
        .entitlements
        .as_mut()
        .expect("entitlements")
        .monthly_ai_credits = 0;

    settings_service::reconcile_provider_policy(&state, &account)
        .await
        .expect("reconcile");

    assert_eq!(ai_mode(&state).await, "offline");
}

#[tokio::test]
async fn first_eligible_account_enables_sync_by_default() {
    let (state, _dir) = fresh_state().await;
    let account = signed_in_account(true, true);

    state
        .cloud
        .reconcile_sync_policy(&account)
        .await
        .expect("sync policy");

    assert!(state.cloud.sync_enabled().await.expect("sync setting"));
}

#[tokio::test]
async fn explicit_sync_disable_is_not_reopened_by_account_login() {
    let (state, _dir) = fresh_state().await;
    let account = signed_in_account(true, true);

    state
        .cloud
        .set_sync_enabled(false)
        .await
        .expect("explicitly disable sync");
    state
        .cloud
        .reconcile_sync_policy(&account)
        .await
        .expect("sync policy");

    assert!(!state.cloud.sync_enabled().await.expect("sync setting"));
}
