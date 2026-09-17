use crate::error::AppError;
use crate::search::traits::{SearchQuery, SearchResult};
use crate::state::AppState;

pub struct SearchService<'a> {
    state: &'a AppState,
}

impl<'a> SearchService<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub async fn search_web(
        &self,
        query: &str,
        count: usize,
    ) -> Result<Vec<SearchResult>, AppError> {
        let mut search_query = SearchQuery::new(query);
        search_query.count = count.clamp(1, 20);
        self.state.search.search(&search_query).await
    }
}
