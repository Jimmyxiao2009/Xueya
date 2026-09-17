use super::retrieval::RetrievedChunk;

pub fn lexical_overlap(query: &str, text: &str) -> f32 {
    let query_terms: Vec<String> = query
        .to_lowercase()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    if query_terms.is_empty() {
        return 0.0;
    }
    let text = text.to_lowercase();
    let matches = query_terms
        .iter()
        .filter(|term| text.contains(term.as_str()))
        .count() as f32;
    matches / query_terms.len() as f32
}

pub fn rerank(mut chunks: Vec<RetrievedChunk>, query: &str) -> Vec<RetrievedChunk> {
    for chunk in &mut chunks {
        let lexical = lexical_overlap(query, &chunk.chunk.text);
        chunk.score = chunk.score * 0.75 + lexical * 0.25;
    }
    chunks.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    chunks
}
