use crate::config::ObjectStorageConfig;
use chrono::Utc;
use hmac::{Hmac, Mac};
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use reqwest::Method;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;
use url::Url;
use uuid::Uuid;

const S3_SERVICE: &str = "s3";
const UNSIGNED_PAYLOAD: &str = "UNSIGNED-PAYLOAD";

const QUERY_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'&')
    .add(b'+')
    .add(b'/')
    .add(b':')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}')
    .add(b';');

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("invalid object storage configuration: {0}")]
    Config(String),
    #[error("object storage request failed")]
    Request,
    #[error("object storage returned status {0}: {1}")]
    Status(reqwest::StatusCode, String),
    #[error("object storage URL is invalid")]
    InvalidUrl,
}

#[derive(Debug, Clone)]
pub struct PresignedRequest {
    pub url: String,
    pub method: String,
    pub headers: BTreeMap<String, String>,
    pub expires_in: i64,
}

#[derive(Clone)]
pub struct S3ObjectStorage {
    config: ObjectStorageConfig,
    client: reqwest::Client,
}

impl S3ObjectStorage {
    pub fn new(config: ObjectStorageConfig) -> Result<Self, StorageError> {
        if !config.is_configured() {
            return Err(StorageError::Config(
                "endpoint, bucket, access_key and secret_key are required".into(),
            ));
        }
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|_| StorageError::Request)?;
        Ok(Self { config, client })
    }

    pub fn config(&self) -> &ObjectStorageConfig {
        &self.config
    }

    pub fn object_key(&self, user_id: Uuid, sha256: &str) -> String {
        format!("users/{user_id}/objects/{sha256}")
    }

    pub fn presign_put(
        &self,
        key: &str,
        content_type: Option<&str>,
    ) -> Result<PresignedRequest, StorageError> {
        self.presign(Method::PUT, key, content_type)
    }

    pub fn presign_get(&self, key: &str) -> Result<PresignedRequest, StorageError> {
        self.presign(Method::GET, key, None)
    }

    pub fn presign_verified_put(
        &self,
        key: &str,
        content_type: &str,
        size: i64,
        sha256: &str,
    ) -> Result<PresignedRequest, StorageError> {
        let mut headers = BTreeMap::new();
        headers.insert("content-type".into(), content_type.into());
        headers.insert("content-length".into(), size.to_string());
        headers.insert("x-amz-content-sha256".into(), sha256.to_ascii_lowercase());
        self.presign_headers(Method::PUT, key, headers, &sha256.to_ascii_lowercase())
    }

    /// Verify storage, with a cumulative budget, before publishing an object.
    pub async fn verify_object(
        &self,
        key: &str,
        size: i64,
        sha256: &str,
    ) -> Result<bool, StorageError> {
        let mut response = self
            .client
            .get(self.presign_get(key)?.url)
            .send()
            .await
            .map_err(|_| StorageError::Request)?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(false);
        }
        if !response.status().is_success() {
            return Err(StorageError::Request);
        }
        let mut count = 0u64;
        let mut hash = Sha256::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| StorageError::Request)? {
            count = count.saturating_add(chunk.len() as u64);
            if count > size as u64 {
                return Ok(false);
            }
            hash.update(&chunk);
        }
        Ok(count == size as u64 && hex::encode(hash.finalize()).eq_ignore_ascii_case(sha256))
    }

    pub async fn ensure_bucket(&self) -> Result<(), StorageError> {
        let (uri, host, scheme) = self.build_bucket_location()?;
        let now = Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = now.format("%Y%m%d").to_string();
        let scope = format!(
            "{date_stamp}/{}/{S3_SERVICE}/aws4_request",
            self.config.region
        );
        let payload_hash = sha256_hex(b"");
        let canonical_headers =
            format!("host:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n");
        let signed_headers = "host;x-amz-content-sha256;x-amz-date";
        let canonical_request =
            format!("PUT\n{uri}\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}");
        let hashed_canonical = sha256_hex(canonical_request.as_bytes());
        let string_to_sign = format!("AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{hashed_canonical}");
        let signing_key = derive_signing_key(
            &self.config.secret_key,
            &date_stamp,
            &self.config.region,
            S3_SERVICE,
        );
        let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes()));
        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={signed_headers}, Signature={signature}",
            self.config.access_key, scope
        );
        let url = format!("{scheme}://{host}{uri}");
        let response = self
            .client
            .put(url)
            .header("x-amz-date", amz_date)
            .header("x-amz-content-sha256", payload_hash)
            .header("Authorization", authorization)
            .send()
            .await
            .map_err(|_| StorageError::Request)?;

        if response.status().is_success() || response.status().as_u16() == 409 {
            return Ok(());
        }
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(StorageError::Status(status, truncate(body, 512)))
    }

    pub async fn delete_object(&self, key: &str) -> Result<(), StorageError> {
        let (uri, host, scheme) = self.build_location(key)?;
        let now = Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = now.format("%Y%m%d").to_string();
        let scope = format!(
            "{date_stamp}/{}/{S3_SERVICE}/aws4_request",
            self.config.region
        );
        let payload_hash = sha256_hex(b"");
        let canonical_headers =
            format!("host:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n");
        let signed_headers = "host;x-amz-content-sha256;x-amz-date";
        let canonical_request =
            format!("DELETE\n{uri}\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}");
        let hashed_canonical = sha256_hex(canonical_request.as_bytes());
        let string_to_sign = format!("AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{hashed_canonical}");
        let signing_key = derive_signing_key(
            &self.config.secret_key,
            &date_stamp,
            &self.config.region,
            S3_SERVICE,
        );
        let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes()));
        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={signed_headers}, Signature={signature}",
            self.config.access_key, scope
        );
        let url = format!("{scheme}://{host}{uri}");
        let response = self
            .client
            .delete(url)
            .header("x-amz-date", amz_date)
            .header("x-amz-content-sha256", payload_hash)
            .header("Authorization", authorization)
            .send()
            .await
            .map_err(|_| StorageError::Request)?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(StorageError::Status(status, truncate(body, 512)));
        }
        Ok(())
    }

    fn presign(
        &self,
        method: Method,
        key: &str,
        content_type: Option<&str>,
    ) -> Result<PresignedRequest, StorageError> {
        let mut headers = BTreeMap::new();
        if let Some(value) = content_type {
            headers.insert("content-type".into(), value.into());
        }
        self.presign_headers(method, key, headers, UNSIGNED_PAYLOAD)
    }

    fn presign_headers(
        &self,
        method: Method,
        key: &str,
        mut headers: BTreeMap<String, String>,
        payload_hash: &str,
    ) -> Result<PresignedRequest, StorageError> {
        let (uri, host, scheme) = self.build_location(key)?;
        let now = Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = now.format("%Y%m%d").to_string();
        let scope = format!(
            "{date_stamp}/{}/{S3_SERVICE}/aws4_request",
            self.config.region
        );
        let ttl = self.config.presign_ttl_secs.max(60);

        headers.insert("host".into(), host.clone());
        let signed_headers = headers.keys().cloned().collect::<Vec<_>>().join(";");
        let canonical_headers = headers
            .iter()
            .map(|(key, value)| format!("{key}:{value}\n"))
            .collect::<String>();

        let mut query_params: Vec<(String, String)> = vec![
            ("X-Amz-Algorithm".into(), "AWS4-HMAC-SHA256".into()),
            (
                "X-Amz-Credential".into(),
                format!("{}/{scope}", self.config.access_key),
            ),
            ("X-Amz-Date".into(), amz_date.clone()),
            ("X-Amz-Expires".into(), ttl.to_string()),
            ("X-Amz-SignedHeaders".into(), signed_headers.clone()),
        ];
        let canonical_query = canonical_query_string(&query_params);
        let canonical_request = format!(
            "{method}\n{uri}\n{canonical_query}\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
        );
        let hashed_canonical = sha256_hex(canonical_request.as_bytes());
        let string_to_sign = format!("AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{hashed_canonical}");
        let signing_key = derive_signing_key(
            &self.config.secret_key,
            &date_stamp,
            &self.config.region,
            S3_SERVICE,
        );
        let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes()));
        query_params.push(("X-Amz-Signature".into(), signature));
        let query = canonical_query_string(&query_params);
        let url = format!("{scheme}://{host}{uri}?{query}");

        headers.remove("host");
        // Keep the public response shape stable; SigV4 uses lowercase names.
        if let Some(content_type) = headers.remove("content-type") {
            headers.insert("Content-Type".into(), content_type);
        }

        Ok(PresignedRequest {
            url,
            method: method.as_str().to_string(),
            headers,
            expires_in: ttl as i64,
        })
    }

    fn build_location(&self, key: &str) -> Result<(String, String, String), StorageError> {
        let endpoint = Url::parse(&self.config.endpoint).map_err(|_| StorageError::InvalidUrl)?;
        let scheme = endpoint.scheme().to_string();
        let base_host = host_with_port(&endpoint);
        let base_path = endpoint.path().trim_end_matches('/');

        let encoded_key = encode_path(key);
        let (host, uri) = if self.config.force_path_style {
            let host = base_host;
            let uri = format!(
                "{}/{}/{}",
                base_path,
                encode_segment(&self.config.bucket),
                encoded_key
            );
            (host, uri)
        } else {
            let host = format!("{}.{}", self.config.bucket, base_host);
            let uri = format!("{}/{}", base_path, encoded_key);
            (host, uri)
        };
        let uri = if uri.starts_with('/') {
            uri
        } else {
            format!("/{uri}")
        };
        Ok((uri, host, scheme))
    }
    fn build_bucket_location(&self) -> Result<(String, String, String), StorageError> {
        let endpoint = Url::parse(&self.config.endpoint).map_err(|_| StorageError::InvalidUrl)?;
        let scheme = endpoint.scheme().to_string();
        let base_host = host_with_port(&endpoint);
        let base_path = endpoint.path().trim_end_matches('/');

        let (host, uri) = if self.config.force_path_style {
            let host = base_host;
            let uri = format!("{}/{}", base_path, encode_segment(&self.config.bucket));
            (host, uri)
        } else {
            let host = format!("{}.{}", self.config.bucket, base_host);
            let uri = if base_path.is_empty() {
                "/".to_string()
            } else {
                base_path.to_string()
            };
            (host, uri)
        };
        let uri = if uri.starts_with('/') {
            uri
        } else {
            format!("/{uri}")
        };
        Ok((uri, host, scheme))
    }
}

fn host_with_port(url: &Url) -> String {
    match url.port() {
        Some(port) => format!("{}:{port}", url.host_str().unwrap_or_default()),
        None => url.host_str().unwrap_or_default().to_string(),
    }
}

fn canonical_query_string(params: &[(String, String)]) -> String {
    let mut params = params.to_vec();
    params.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    params
        .iter()
        .map(|(key, value)| {
            format!(
                "{}={}",
                encode_query_component(key),
                encode_query_component(value)
            )
        })
        .collect::<Vec<_>>()
        .join("&")
}

fn encode_query_component(value: &str) -> String {
    utf8_percent_encode(value, QUERY_ENCODE_SET).to_string()
}

fn encode_segment(value: &str) -> String {
    encode_path(value)
}

fn encode_path(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect::<Vec<_>>()
        .join("")
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("hmac accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn derive_signing_key(secret: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date = hmac_sha256(format!("AWS4{secret}").as_bytes(), date.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    hmac_sha256(&k_service, b"aws4_request")
}

fn truncate(value: String, max: usize) -> String {
    if value.len() <= max {
        value
    } else {
        format!("{}...", &value[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> ObjectStorageConfig {
        ObjectStorageConfig {
            endpoint: "http://localhost:9000".into(),
            region: "us-east-1".into(),
            bucket: "verruckt".into(),
            access_key: "minioadmin".into(),
            secret_key: "minioadmin".into(),
            force_path_style: true,
            presign_ttl_secs: 900,
        }
    }

    #[test]
    fn object_keys_never_use_filenames() {
        let storage = S3ObjectStorage::new(config()).unwrap();
        let user_id = Uuid::nil();
        let key = storage.object_key(user_id, &"a".repeat(64));
        assert_eq!(key, format!("users/{user_id}/objects/{}", "a".repeat(64)));
    }

    #[test]
    fn presign_put_contains_signature_and_scope() {
        let storage = S3ObjectStorage::new(config()).unwrap();
        let presigned = storage
            .presign_put("users/x/objects/abc", Some("application/pdf"))
            .unwrap();
        assert_eq!(presigned.method, "PUT");
        assert!(presigned.url.contains("X-Amz-Algorithm=AWS4-HMAC-SHA256"));
        assert!(presigned.url.contains("X-Amz-Signature="));
        assert!(presigned
            .url
            .contains("X-Amz-SignedHeaders=content-type%3Bhost"));
        assert!(presigned.url.contains("verruckt/users/x/objects/abc"));
        assert_eq!(
            presigned.headers.get("Content-Type").map(String::as_str),
            Some("application/pdf")
        );
    }

    #[test]
    fn presign_get_uses_get_method() {
        let storage = S3ObjectStorage::new(config()).unwrap();
        let presigned = storage.presign_get("users/x/objects/abc").unwrap();
        assert_eq!(presigned.method, "GET");
        assert!(!presigned.url.contains("content-type"));
    }
}
