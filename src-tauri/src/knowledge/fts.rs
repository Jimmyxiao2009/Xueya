use crate::error::AppError;
use sqlx::SqlitePool;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct LexicalHit {
    pub chunk_id: Uuid,
    pub score: f32,
}

/// Build a safe FTS5 MATCH expression from arbitrary learner text.
///
/// FTS5 query syntax is user-hostile: a stray `"`, `*`, `-` or `:` turns a
/// normal query into an error or an unintended operator. We extract
/// alphanumeric terms and quote each one, then require all terms to match.
fn fts_expression(query: &str) -> Option<String> {
    let mut terms = Vec::new();
    for term in query
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| !term.trim().is_empty())
    {
        let term = term.trim();
        if term.is_empty() {
            continue;
        }
        let escaped = term.replace('"', "\"\"");
        terms.push(format!("\"{escaped}\""));
    }
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" AND "))
    }
}

pub async fn lexical_search(
    pool: &SqlitePool,
    query: &str,
    limit: usize,
    allowed_chunk_ids: Option<&HashSet<Uuid>>,
) -> Result<Vec<LexicalHit>, AppError> {
    if let Some(allowed) = allowed_chunk_ids {
        if allowed.is_empty() {
            return Ok(Vec::new());
        }
    }
    let Some(expression) = fts_expression(query) else {
        return Ok(Vec::new());
    };
    let limit = limit.clamp(1, 100);
    let mut sql = String::from(
        "SELECT chunk_id, -bm25(chunk_fts) AS score FROM chunk_fts \
         WHERE chunk_fts MATCH ?",
    );
    if let Some(allowed) = allowed_chunk_ids {
        if !allowed.is_empty() {
            let placeholders = vec!["?"; allowed.len()].join(",");
            sql.push_str(&format!(" AND chunk_id IN ({placeholders})"));
        }
    }
    sql.push_str(" ORDER BY score DESC LIMIT ?");

    let mut statement = sqlx::query_as::<_, (Vec<u8>, f64)>(&sql).bind(expression);
    if let Some(allowed) = allowed_chunk_ids {
        for chunk_id in allowed {
            statement = statement.bind(chunk_id);
        }
    }
    let rows = statement.bind(limit as i64).fetch_all(pool).await?;
    let mut hits: Vec<LexicalHit> = rows
        .into_iter()
        .filter_map(|(chunk_id, score)| {
            Uuid::from_slice(&chunk_id).ok().map(|chunk_id| LexicalHit {
                chunk_id,
                score: score as f32,
            })
        })
        .collect();
    hits.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if hits.is_empty() {
        return like_search(pool, query, limit, allowed_chunk_ids).await;
    }
    Ok(hits)
}

/// Literal substring fallback for CJK text and IDs such as `[1:2:0]` where the
/// unicode61 tokenizer may not produce useful tokens.
async fn like_search(
    pool: &SqlitePool,
    query: &str,
    limit: usize,
    allowed_chunk_ids: Option<&HashSet<Uuid>>,
) -> Result<Vec<LexicalHit>, AppError> {
    let needle = query.trim();
    if needle.is_empty() {
        return Ok(Vec::new());
    }
    let pattern = format!("%{needle}%");
    let mut sql = String::from("SELECT id FROM chunks WHERE text LIKE ?");
    if let Some(allowed) = allowed_chunk_ids {
        if allowed.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = vec!["?"; allowed.len()].join(",");
        sql.push_str(&format!(" AND id IN ({placeholders})"));
    }
    sql.push_str(" LIMIT ?");
    let mut statement = sqlx::query_scalar::<_, Uuid>(&sql).bind(pattern);
    if let Some(allowed) = allowed_chunk_ids {
        for chunk_id in allowed {
            statement = statement.bind(chunk_id);
        }
    }
    let rows = statement.bind(limit as i64).fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .enumerate()
        .map(|(rank, chunk_id)| LexicalHit {
            chunk_id,
            score: 1.0 / (1.0 + rank as f32),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_safe_fts_expressions() {
        assert_eq!(
            fts_expression("[1:2:0]").as_deref(),
            Some("\"1\" AND \"2\" AND \"0\"")
        );
        assert_eq!(
            fts_expression("point at infinity").as_deref(),
            Some("\"point\" AND \"at\" AND \"infinity\"")
        );
        assert!(fts_expression("!!!").is_none());
    }
}
