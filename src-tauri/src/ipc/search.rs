use super::dto::{SearchResultDto, SearchWebInput};
use crate::error::AppError;
use crate::services::search_service::SearchService;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn search_web(
    state: State<'_, AppState>,
    input: SearchWebInput,
) -> Result<Vec<SearchResultDto>, AppError> {
    let results = SearchService::new(&state)
        .search_web(&input.query, input.count.unwrap_or(10))
        .await?;
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
