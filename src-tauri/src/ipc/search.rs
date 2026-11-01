use super::dto::{SearchResultDto, SearchWebInput};
use crate::error::AppError;
use crate::ipc::State;
use crate::services::search_service::SearchService;
use crate::state::AppState;

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn search_web(
    state: State<'_, AppState>,
    input: SearchWebInput,
) -> Result<Vec<SearchResultDto>, AppError> {
    let service = SearchService::new(&state);
    let results = if input.ai { service.discover(&input.query, input.count.unwrap_or(6)).await? }
        else { service.search_web(&input.query, input.count.unwrap_or(10)).await? };
    Ok(results
        .into_iter()
        .map(|result| SearchResultDto {
            title: result.title,
            url: result.url,
            snippet: result.snippet,
            source: result.source,
            published_at: result.published_at,
        })
        .collect())
}
