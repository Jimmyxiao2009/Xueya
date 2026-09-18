pub mod config;
pub mod s3;

pub use config::ObjectStorageConfig;
pub use s3::{S3ObjectStorage, StorageError};
