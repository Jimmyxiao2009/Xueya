use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectStorageConfig {
    /// S3-compatible endpoint, e.g. http://minio:9000 or https://s3.amazonaws.com.
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    /// MinIO is path-style. Production S3/R2 can use virtual-hosted style.
    pub force_path_style: bool,
    pub presign_ttl_secs: u64,
}

impl ObjectStorageConfig {
    pub fn is_configured(&self) -> bool {
        !self.endpoint.trim().is_empty()
            && !self.bucket.trim().is_empty()
            && !self.access_key.trim().is_empty()
            && !self.secret_key.trim().is_empty()
    }
}
