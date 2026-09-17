pub mod ai;
pub mod bootstrap;
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
pub mod util;

use ipc::{
    app::{app_bootstrap, app_get_info},
    courses::{
        course_create, course_delete, course_get, course_list, course_node_get,
        course_node_prepare, course_regenerate,
    },
    jobs::{job_cancel, job_get, job_list, job_subscribe},
    knowledge::{knowledge_get_object, knowledge_get_related, knowledge_search},
    learning::{learning_finish, learning_respond, learning_start},
    providers::{provider_delete, provider_list, provider_test, provider_upsert},
    search::search_web,
    settings::{settings_get, settings_update},
    sources::{
        source_delete, source_import_bytes, source_import_files, source_import_url, source_list,
        source_reindex,
    },
};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    let result = tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            let state = tauri::async_runtime::block_on(bootstrap::build_state(&handle))?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_bootstrap,
            app_get_info,
            settings_get,
            settings_update,
            provider_list,
            provider_upsert,
            provider_delete,
            provider_test,
            source_list,
            source_import_bytes,
            source_import_files,
            source_import_url,
            source_delete,
            source_reindex,
            search_web,
            knowledge_search,
            knowledge_get_object,
            knowledge_get_related,
            course_list,
            course_get,
            course_create,
            course_delete,
            course_regenerate,
            course_node_get,
            course_node_prepare,
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
