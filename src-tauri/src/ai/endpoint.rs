use crate::error::AppError;
use url::Url;

/// Normalize a user-supplied OpenAI-compatible base URL.
///
/// Users paste all of these and mean the same thing:
///
/// * `https://example.com`
/// * `https://example.com/`
/// * `https://example.com/v1`
/// * `https://example.com/v1/`
///
/// The returned value never ends in `/` and never contains a duplicate `/v1`.
pub fn normalize_base_url(base_url: &str) -> Result<String, AppError> {
    let raw = base_url.trim();
    if raw.is_empty() {
        return Err(AppError::InvalidInput("provider base URL is empty".into()));
    }
    let mut url = Url::parse(raw)
        .map_err(|error| AppError::InvalidInput(format!("invalid provider base URL: {error}")))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::InvalidInput(
            "provider base URL must use http or https".into(),
        ));
    }
    let path = url.path().trim_end_matches('/').to_owned();
    let path = if path.is_empty() {
        "/".to_owned()
    } else {
        path
    };
    url.set_path(&path);
    url.set_query(None);
    url.set_fragment(None);
    let mut normalized = url.to_string();
    if normalized.ends_with('/') && normalized.len() > "https://x/".len() {
        normalized.pop();
    }
    Ok(normalized)
}

fn has_version_segment(path: &str) -> bool {
    path.split('/').any(|segment| segment == "v1")
}

/// Join a normalized base URL with an API resource path.
///
/// The version segment is inserted exactly once. This is the only place in the
/// app that is allowed to build provider URLs, so `/v1/v1/embeddings` and
/// `/v1//embeddings` are structurally impossible.
pub fn endpoint_url(base_url: &str, resource: &str) -> Result<String, AppError> {
    let base = normalize_base_url(base_url)?;
    let resource = resource.trim().trim_start_matches('/');
    if resource.is_empty() {
        return Ok(base);
    }
    let resource = resource.strip_prefix("v1/").unwrap_or(resource);
    let resource = resource.strip_prefix("v1").unwrap_or(resource);
    let resource = resource.trim_start_matches('/');

    let mut parsed = Url::parse(&base)
        .map_err(|error| AppError::InvalidInput(format!("invalid provider base URL: {error}")))?;
    let base_path = parsed.path().trim_end_matches('/').to_owned();
    let joined_path = if has_version_segment(&base_path) {
        format!("{base_path}/{resource}")
    } else if base_path.is_empty() || base_path == "/" {
        format!("/v1/{resource}")
    } else {
        format!("{base_path}/v1/{resource}")
    };
    parsed.set_path(&joined_path);
    parsed.set_query(None);
    parsed.set_fragment(None);
    Ok(parsed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_host_and_v1_variants() {
        assert_eq!(
            normalize_base_url("https://example.com/").expect("url"),
            "https://example.com"
        );
        assert_eq!(
            normalize_base_url("https://example.com/v1/").expect("url"),
            "https://example.com/v1"
        );
    }

    #[test]
    fn builds_endpoints_without_duplicating_v1_or_slashes() {
        for base in [
            "https://example.com",
            "https://example.com/",
            "https://example.com/v1",
            "https://example.com/v1/",
        ] {
            let url = endpoint_url(base, "embeddings").expect("url");
            assert_eq!(url, "https://example.com/v1/embeddings", "base: {base}");
        }
        let nested =
            endpoint_url("https://example.com/openai/v1", "chat/completions").expect("nested url");
        assert_eq!(nested, "https://example.com/openai/v1/chat/completions");
    }

    #[test]
    fn strips_resource_side_v1_to_avoid_v1_v1() {
        let url = endpoint_url("https://example.com/v1", "v1/embeddings").expect("url");
        assert_eq!(url, "https://example.com/v1/embeddings");
        assert!(!url.contains("/v1/v1/"));
    }
}
