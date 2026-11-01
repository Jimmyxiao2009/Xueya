use crate::error::AppError;
use reqwest::header::LOCATION;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;
use url::Url;

pub const DEFAULT_MAX_REDIRECTS: usize = 5;
pub const DEFAULT_MAX_BYTES: usize = 25 * 1024 * 1024;
pub const DEFAULT_TIMEOUT_SECONDS: u64 = 30;

#[derive(Debug, Clone, Copy)]
pub struct UrlSecurityPolicy {
    pub max_redirects: usize,
    pub max_bytes: usize,
    pub timeout: Duration,
}

impl Default for UrlSecurityPolicy {
    fn default() -> Self {
        Self {
            max_redirects: DEFAULT_MAX_REDIRECTS,
            max_bytes: DEFAULT_MAX_BYTES,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECONDS),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FetchedUrl {
    pub final_url: Url,
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
}

/// Validate URL syntax before any network work happens.
pub fn validate_url_syntax(url: &Url) -> Result<(), AppError> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::InvalidInput(
            "only http/https URLs can be imported".into(),
        ));
    }
    if url.username() != "" || url.password().is_some() {
        return Err(AppError::InvalidInput(
            "URLs with embedded credentials are not allowed".into(),
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| AppError::InvalidInput("URL has no host".into()))?;
    let lower = host.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "localhost" | "localhost.localdomain" | "0.0.0.0" | "[::]"
    ) {
        return Err(AppError::InvalidInput(
            "local or unspecified hosts are not allowed".into(),
        ));
    }
    Ok(())
}

pub fn is_forbidden_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_forbidden_ipv4(ip),
        IpAddr::V6(ip) => is_forbidden_ipv6(ip),
    }
}

fn is_forbidden_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ip.is_documentation()
        || (octets[0] == 100 && (64..=127).contains(&octets[1])) // CGNAT
        || matches!(octets, [169, 254, 169, 254]) // cloud metadata
        || matches!(octets, [100, 100, 100, 200]) // Alibaba metadata
}

fn is_forbidden_ipv6(ip: Ipv6Addr) -> bool {
    ip.to_ipv4_mapped().is_some_and(is_forbidden_ipv4)
        || ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || is_unique_local(ip)
        || is_unicast_link_local(ip)
        || is_documentation_v6(ip)
        || matches!(ip.segments(), [0xfd00, 0x0ec2, 0, 0, 0, 0, 0, 0x254]) // AWS metadata IPv6
}

fn is_unique_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

fn is_unicast_link_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

fn is_documentation_v6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    segments[0] == 0x2001 && segments[1] == 0x0db8
}

/// Resolve a hostname and reject every address that points at the local
/// machine, a private network or a cloud metadata service.
pub async fn validate_resolved(url: &Url) -> Result<(), AppError> {
    resolved_addresses(url).await.map(|_| ())
}

async fn resolved_addresses(url: &Url) -> Result<Vec<std::net::SocketAddr>, AppError> {
    validate_url_syntax(url)?;
    let host = url
        .host_str()
        .ok_or_else(|| AppError::InvalidInput("URL has no host".into()))?
        .trim_matches(['[', ']']);
    let port = url
        .port_or_known_default()
        .ok_or_else(|| AppError::InvalidInput("URL has no usable port".into()))?;

    // Literal IPs can be checked without DNS.
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_forbidden_ip(ip) {
            return Err(AppError::InvalidInput(
                "URL resolves to a blocked private or local address".into(),
            ));
        }
        return Ok(vec![std::net::SocketAddr::new(ip, port)]);
    }

    let addresses: Vec<_> = tokio::net::lookup_host((host, port))
        .await
        .map_err(|error| AppError::Network(format!("DNS resolution failed: {error}")))?
        .collect();
    let mut any = false;
    for address in &addresses {
        any = true;
        if is_forbidden_ip(address.ip()) {
            return Err(AppError::InvalidInput(
                "URL resolves to a blocked private or local address".into(),
            ));
        }
    }
    if !any {
        return Err(AppError::Network(
            "DNS resolution returned no addresses".into(),
        ));
    }
    Ok(addresses)
}

fn allowed_content_type(content_type: Option<&str>) -> bool {
    let Some(value) = content_type else {
        return true;
    };
    let value = value.to_ascii_lowercase();
    value.starts_with("text/")
        || value.contains("application/pdf")
        || value.contains("application/markdown")
        || value.contains("application/xhtml")
        || value.contains("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        || value
            .contains("application/vnd.openxmlformats-officedocument.presentationml.presentation")
        || value.contains("application/octet-stream")
}

/// Fetch a URL with manual redirect handling. Every hop is validated again, so
/// a public URL cannot redirect into `127.0.0.1` or cloud metadata.
pub async fn fetch_safely(
    _client: &reqwest::Client,
    start: &Url,
    policy: UrlSecurityPolicy,
) -> Result<FetchedUrl, AppError> {
    let mut current = start.clone();
    let mut redirects = 0usize;
    loop {
        validate_url_syntax(&current)?;
        let addresses = resolved_addresses(&current).await?;
        let host = current
            .host_str()
            .ok_or_else(|| AppError::InvalidInput("URL has no host".into()))?
            .trim_matches(['[', ']']);
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .resolve_to_addrs(host, &addresses)
            .timeout(policy.timeout)
            .build()
            .map_err(|e| AppError::Network(e.to_string()))?;
        let mut response = client
            .get(current.clone())
            .timeout(policy.timeout)
            .send()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;

        if response.status().is_redirection() {
            if redirects >= policy.max_redirects {
                return Err(AppError::InvalidInput("too many URL redirects".into()));
            }
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| AppError::Network("redirect response had no Location".into()))?;
            let next = current.join(location).map_err(|error| {
                AppError::InvalidInput(format!("invalid redirect URL: {error}"))
            })?;
            current = next;
            redirects += 1;
            continue;
        }

        if !response.status().is_success() {
            return Err(AppError::Network(format!(
                "URL import returned {}",
                response.status()
            )));
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        if !allowed_content_type(content_type.as_deref()) {
            return Err(AppError::UnsupportedFormat(format!(
                "unsupported remote content type: {}",
                content_type.unwrap_or_default()
            )));
        }
        if let Some(length) = response.content_length() {
            if length as usize > policy.max_bytes {
                return Err(AppError::InvalidInput(format!(
                    "remote document too large: {length} bytes"
                )));
            }
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?
        {
            if chunk.len() > policy.max_bytes.saturating_sub(bytes.len()) {
                return Err(AppError::InvalidInput("remote document too large".into()));
            }
            bytes.extend_from_slice(&chunk);
        }
        return Ok(FetchedUrl {
            final_url: current,
            content_type,
            bytes,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_private_and_metadata_addresses() {
        assert!(is_forbidden_ip("127.0.0.1".parse().unwrap()));
        assert!(is_forbidden_ip("10.0.0.1".parse().unwrap()));
        assert!(is_forbidden_ip("192.168.1.10".parse().unwrap()));
        assert!(is_forbidden_ip("169.254.169.254".parse().unwrap()));
        assert!(is_forbidden_ip("::1".parse().unwrap()));
        assert!(!is_forbidden_ip("93.184.216.34".parse().unwrap()));
    }

    #[test]
    fn rejects_non_http_and_credential_urls() {
        let file = Url::parse("file:///etc/passwd").expect("url");
        assert!(validate_url_syntax(&file).is_err());
        let ftp = Url::parse("ftp://example.com/file").expect("url");
        assert!(validate_url_syntax(&ftp).is_err());
        let creds = Url::parse("https://user:pass@example.com").expect("url");
        assert!(validate_url_syntax(&creds).is_err());
    }
}
