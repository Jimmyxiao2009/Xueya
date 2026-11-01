use crate::error::AppError;
use crate::util::hashing::sha256_hex;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone)]
pub struct StoredBlob {
    pub hash: String,
    pub relative_path: String,
    pub size_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct BlobStore {
    root: PathBuf,
}

impl BlobStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn relative_path_for_hash(hash: &str) -> String {
        let prefix = &hash[..2.min(hash.len())];
        format!("{prefix}/{hash}")
    }

    fn relative_path(hash: &str) -> String {
        Self::relative_path_for_hash(hash)
    }

    pub fn full_path(&self, relative_path: &str) -> PathBuf {
        self.root.join(relative_path)
    }

    pub async fn put_bytes(&self, bytes: &[u8]) -> Result<StoredBlob, AppError> {
        let hash = sha256_hex(bytes);
        let relative_path = Self::relative_path(&hash);
        let destination = self.full_path(&relative_path);
        let valid_existing = match self.get_bytes(&relative_path).await {
            Ok(existing) => existing.len() == bytes.len() && sha256_hex(&existing) == hash,
            Err(_) => false,
        };
        if valid_existing {
            return Ok(StoredBlob {
                hash,
                relative_path,
                size_bytes: bytes.len(),
            });
        }
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let temporary = destination.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        let mut file = tokio::fs::File::create(&temporary).await?;
        file.write_all(bytes).await?;
        file.sync_all().await?;
        drop(file);
        match tokio::fs::rename(&temporary, &destination).await {
            Ok(()) => {}
            Err(error) => return Err(AppError::Io(error.to_string())),
        }
        Ok(StoredBlob {
            hash,
            relative_path,
            size_bytes: bytes.len(),
        })
    }

    pub async fn get_bytes(&self, relative_path: &str) -> Result<Vec<u8>, AppError> {
        if relative_path.contains("..") {
            return Err(AppError::InvalidInput(
                "blob path traversal rejected".into(),
            ));
        }
        use tokio::io::AsyncReadExt;
        const MAX_BYTES: usize = 50 * 1024 * 1024;
        let file = tokio::fs::File::open(self.full_path(relative_path)).await?;
        let mut bytes = Vec::new();
        file.take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .await?;
        if bytes.len() > MAX_BYTES {
            return Err(AppError::InvalidInput("blob exceeds import limit".into()));
        }
        Ok(bytes)
    }

    pub async fn exists(&self, relative_path: &str) -> bool {
        tokio::fs::try_exists(self.full_path(relative_path))
            .await
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn deduplicates_identical_content() {
        let directory = tempfile::tempdir().expect("tempdir");
        let store = BlobStore::new(directory.path());
        let first = store.put_bytes(b"same content").await.expect("put");
        let second = store.put_bytes(b"same content").await.expect("put");
        assert_eq!(first.hash, second.hash);
        assert_eq!(first.relative_path, second.relative_path);
        assert_eq!(
            store.get_bytes(&first.relative_path).await.expect("read"),
            b"same content"
        );
    }
}
