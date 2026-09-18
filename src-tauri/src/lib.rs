pub mod ai;
pub mod bootstrap;
pub mod cloud;
pub mod curriculum;
pub mod db;
pub mod domain;
pub mod error;
pub mod ingest;
pub mod ipc;
pub mod jobs;
pub mod knowledge;
pub mod knowledge_fs;
pub mod learning;
pub mod search;
pub mod secrets;
pub mod services;
pub mod state;
pub mod sync;
pub mod util;

use cloud::commands::{
    cloud_account_state, cloud_get_ai_usage, cloud_get_entitlements, cloud_get_me,
    cloud_list_devices, cloud_login, cloud_logout, cloud_register, cloud_remove_device,
    cloud_set_base_url, cloud_set_sync_enabled,
};
use ipc::{
    app::{app_bootstrap, app_get_info},
    courses::{
        course_adaptations, course_create, course_delete, course_get, course_learner_states,
        course_list, course_node_complete, course_node_get, course_node_prepare, course_regenerate,
    },
    jobs::{job_cancel, job_get, job_list, job_subscribe},
    knowledge::{
        knowledge_get_object, knowledge_get_related, knowledge_retrieval_debug, knowledge_search,
    },
    learning::{learning_finish, learning_respond, learning_start},
    providers::{
        provider_delete, provider_list, provider_stream_chat, provider_test, provider_upsert,
    },
    quiz::{quiz_list, quiz_submit},
    search::search_web,
    settings::{settings_get, settings_update},
    sources::{
        source_delete, source_import_bytes, source_import_files, source_import_url, source_list,
        source_reindex, source_summarize,
    },
    tutor::{tutor_send, tutor_send_stream},
};
use services::ingestion_service::IngestionService;
use sync::commands::{sync_conflicts, sync_files_now, sync_now, sync_status};
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    let result = tauri::Builder::default()
        // The window is the app's only chrome, so its geometry is worth
        // keeping: a learner who resizes it once should not have to again.
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            let handle = app.handle().clone();
            let state = tauri::async_runtime::block_on(bootstrap::build_state(&handle))?;
            if let Err(error) = tauri::async_runtime::block_on(bootstrap::seed_demo_routes(&state))
            {
                tracing::warn!(%error, "could not seed the demo routes");
            }
            // The core cloud client stays Tauri-free so headless tests can link
            // it; this is the only place that forwards its events to the WebView.
            let mut cloud_events = state.cloud.subscribe();
            let event_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                while let Ok(event) = cloud_events.recv().await {
                    let _ = event_handle.emit(&event.event, event.payload);
                }
            });
            let sync_state = state.sync.clone();
            tauri::async_runtime::spawn(async move {
                let _ = sync_state.sync_now().await;
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
                let mut last_pending = -1_i64;
                let mut quiet_ticks = 0_u32;
                let mut last_periodic = std::time::Instant::now();
                loop {
                    interval.tick().await;
                    if let Ok(status) = sync_state.status().await {
                        if status.pending_changes != last_pending {
                            last_pending = status.pending_changes;
                            quiet_ticks = 0;
                        } else if status.pending_changes > 0 {
                            quiet_ticks += 1;
                            // Three quiet ticks (about 9s) after the last local
                            // write, push once instead of after every keystroke.
                            if quiet_ticks == 3 {
                                let _ = sync_state.sync_now().await;
                                quiet_ticks = 0;
                            }
                        }
                    }
                    if last_periodic.elapsed() >= std::time::Duration::from_secs(300) {
                        let _ = sync_state.sync_now().await;
                        last_periodic = std::time::Instant::now();
                    }
                }
            });
            let file_state = state.clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
                loop {
                    interval.tick().await;
                    if let Ok(summary) = file_state.files.sync().await {
                        for source_id in summary.reindex_source_ids {
                            if let Ok(source_id) = uuid::Uuid::parse_str(&source_id) {
                                if let Err(error) = IngestionService::new(&file_state)
                                    .reindex_source(source_id, None)
                                    .await
                                {
                                    tracing::warn!(%source_id, %error, "could not reindex a downloaded source");
                                }
                            }
                        }
                    }
                }
            });
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_bootstrap,
            app_get_info,
            cloud_account_state,
            cloud_register,
            cloud_login,
            cloud_logout,
            cloud_get_me,
            cloud_get_entitlements,
            cloud_list_devices,
            cloud_remove_device,
            cloud_get_ai_usage,
            cloud_set_base_url,
            cloud_set_sync_enabled,
            sync_status,
            sync_now,
            sync_conflicts,
            sync_files_now,
            settings_get,
            settings_update,
            provider_list,
            provider_upsert,
            provider_delete,
            provider_test,
            provider_stream_chat,
            source_list,
            source_import_bytes,
            source_import_files,
            source_import_url,
            source_delete,
            source_reindex,
            source_summarize,
            search_web,
            knowledge_search,
            knowledge_retrieval_debug,
            knowledge_get_object,
            knowledge_get_related,
            course_list,
            course_get,
            course_create,
            course_delete,
            course_regenerate,
            course_node_get,
            course_node_prepare,
            course_node_complete,
            course_learner_states,
            course_adaptations,
            quiz_list,
            quiz_submit,
            tutor_send,
            tutor_send_stream,
            learning_start,
            learning_respond,
            learning_finish,
            job_list,
            job_get,
            job_cancel,
            job_subscribe,
        ])
        .run(tauri::generate_context!());

    if let Err(error) = result {
        tracing::error!("Verruckt exited with error: {error}");
    }
}
