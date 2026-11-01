use super::*;
pub async fn dispatch(state: &AppState, request: Invoke) -> Result<Value, AppError> {
    match request.command.as_str() {
        "app_bootstrap" => {
            value(verruckt_lib::ipc::app::app_bootstrap(ipc::State::new(state)).await?)
        }
        "app_get_info" => {
            value(verruckt_lib::ipc::app::app_get_info(ipc::State::new(state)).await?)
        }
        "course_list" => {
            value(verruckt_lib::ipc::courses::course_list(ipc::State::new(state)).await?)
        }
        "course_get" => value(
            verruckt_lib::ipc::courses::course_get(
                ipc::State::new(state),
                arg(&request.args, "id")?,
            )
            .await?,
        ),
        "course_create" => value(
            verruckt_lib::ipc::courses::course_create(
                ipc::State::new(state),
                arg(&request.args, "input")?,
            )
            .await?,
        ),
        "course_regenerate" => value(
            verruckt_lib::ipc::courses::course_regenerate(
                ipc::State::new(state),
                arg(&request.args, "id")?,
            )
            .await?,
        ),
        "course_delete" => value(
            verruckt_lib::ipc::courses::course_delete(
                ipc::State::new(state),
                arg(&request.args, "id")?,
            )
            .await?,
        ),
        "course_node_get" => value(
            verruckt_lib::ipc::courses::course_node_get(
                ipc::State::new(state),
                arg(&request.args, "nodeId")?,
            )
            .await?,
        ),
        "course_node_prepare" => value(
            verruckt_lib::ipc::courses::course_node_prepare(
                ipc::State::new(state),
                arg(&request.args, "nodeId")?,
            )
            .await?,
        ),
        "course_node_complete" => value(
            verruckt_lib::ipc::courses::course_node_complete(
                ipc::State::new(state),
                arg(&request.args, "nodeId")?,
            )
            .await?,
        ),
        "course_learner_states" => value(
            verruckt_lib::ipc::courses::course_learner_states(
                ipc::State::new(state),
                arg(&request.args, "id")?,
            )
            .await?,
        ),
        "course_adaptations" => value(
            verruckt_lib::ipc::courses::course_adaptations(
                ipc::State::new(state),
                arg(&request.args, "id")?,
                arg(&request.args, "limit")?,
            )
            .await?,
        ),
        "source_list" => {
            value(verruckt_lib::ipc::sources::source_list(ipc::State::new(state)).await?)
        }
        "source_summarize" => value(
            verruckt_lib::ipc::sources::source_summarize(
                ipc::State::new(state),
                arg(&request.args, "id")?,
            )
            .await?,
        ),
        "source_import_bytes" => value(
            verruckt_lib::ipc::sources::source_import_bytes(
                ipc::State::new(state),
                arg(&request.args, "input")?,
            )
            .await?,
        ),
        "source_import_url" => value(
            verruckt_lib::ipc::sources::source_import_url(
                ipc::State::new(state),
                arg(&request.args, "input")?,
            )
            .await?,
        ),
        "source_delete" => value(
            verruckt_lib::ipc::sources::source_delete(
                ipc::State::new(state),
                arg(&request.args, "id")?,
            )
            .await?,
        ),
        "source_reindex" => value(
            verruckt_lib::ipc::sources::source_reindex(
                ipc::State::new(state),
                arg(&request.args, "id")?,
            )
            .await?,
        ),
        "job_list" => value(verruckt_lib::ipc::jobs::job_list(ipc::State::new(state)).await?),
        "job_get" => value(
            verruckt_lib::ipc::jobs::job_get(ipc::State::new(state), arg(&request.args, "id")?)
                .await?,
        ),
        "job_cancel" => value(
            verruckt_lib::ipc::jobs::job_cancel(ipc::State::new(state), arg(&request.args, "id")?)
                .await?,
        ),
        "learning_start" => value(
            verruckt_lib::ipc::learning::learning_start(
                ipc::State::new(state),
                arg(&request.args, "input")?,
            )
            .await?,
        ),
        "learning_respond" => value(
            verruckt_lib::ipc::learning::learning_respond(
                ipc::State::new(state),
                arg(&request.args, "input")?,
            )
            .await?,
        ),
        "learning_finish" => value(
            verruckt_lib::ipc::learning::learning_finish(
                ipc::State::new(state),
                arg(&request.args, "input")?,
            )
            .await?,
        ),
        "quiz_list" => value(
            verruckt_lib::ipc::quiz::quiz_list(
                ipc::State::new(state),
                arg(&request.args, "nodeId")?,
            )
            .await?,
        ),
        "quiz_submit" => value(
            verruckt_lib::ipc::quiz::quiz_submit(
                ipc::AppHandle,
                ipc::State::new(state),
                arg(&request.args, "input")?,
            )
            .await?,
        ),
        "tutor_send" => value(
            verruckt_lib::ipc::tutor::tutor_send(
                ipc::AppHandle,
                ipc::State::new(state),
                arg(&request.args, "input")?,
            )
            .await?,
        ),
        "provider_list" => {
            value(verruckt_lib::ipc::providers::provider_list(ipc::State::new(state)).await?)
        }
        "provider_test" => value(
            verruckt_lib::ipc::providers::provider_test(
                ipc::State::new(state),
                arg(&request.args, "id")?,
            )
            .await?,
        ),
        "settings_get" => {
            value(verruckt_lib::ipc::settings::settings_get(ipc::State::new(state)).await?)
        }
        "search_web" => value(
            verruckt_lib::ipc::search::search_web(
                ipc::State::new(state),
                arg(&request.args, "input")?,
            )
            .await?,
        ),
        "cloud_account_state" => {
            value(verruckt_lib::cloud::commands::cloud_account_state(ipc::State::new(state)).await?)
        }
        "cloud_get_me" => {
            value(verruckt_lib::cloud::commands::cloud_get_me(ipc::State::new(state)).await?)
        }
        "cloud_get_entitlements" => value(
            verruckt_lib::cloud::commands::cloud_get_entitlements(ipc::State::new(state)).await?,
        ),
        "cloud_list_devices" => {
            value(verruckt_lib::cloud::commands::cloud_list_devices(ipc::State::new(state)).await?)
        }
        "cloud_remove_device" => value(
            verruckt_lib::cloud::commands::cloud_remove_device(
                ipc::State::new(state),
                arg(&request.args, "id")?,
            )
            .await?,
        ),
        "cloud_get_ai_usage" => {
            value(verruckt_lib::cloud::commands::cloud_get_ai_usage(ipc::State::new(state)).await?)
        }
        "cloud_set_sync_enabled" => value(
            verruckt_lib::cloud::commands::cloud_set_sync_enabled(
                ipc::State::new(state),
                arg(&request.args, "enabled")?,
            )
            .await?,
        ),
        "sync_status" => {
            value(verruckt_lib::sync::commands::sync_status(ipc::State::new(state)).await?)
        }
        "sync_now" => value(verruckt_lib::sync::commands::sync_now(ipc::State::new(state)).await?),
        "sync_conflicts" => {
            value(verruckt_lib::sync::commands::sync_conflicts(ipc::State::new(state)).await?)
        }
        "sync_files_now" => {
            value(verruckt_lib::sync::commands::sync_files_now(ipc::State::new(state)).await?)
        }
        _ => Err(AppError::InvalidInput(
            "此操作不适用于浏览器演示服务".into(),
        )),
    }
}
