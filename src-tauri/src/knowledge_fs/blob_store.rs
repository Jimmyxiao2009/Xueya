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
        if tokio::fs::try_exists(&destination).await.unwrap_or(false) {
            return Ok(StoredBlob {
                hash,
                relative_path,
                size_bytes: bytes.len(),
            });
        }
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let temporary = destination.with_extension("tmp");
        let mut file = tokio::fs::File::create(&temporary).await?;
        file.write_all(bytes).await?;
        file.flush().await?;
        drop(file);
        match tokio::fs::rename(&temporary, &destination).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let _ = tokio::fs::remove_file(&temporary).await;
            }
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
        Ok(tokio::fs::read(self.full_path(relative_path)).await?)
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
