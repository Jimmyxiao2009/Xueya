use thiserror::Error;

/// All errors returned by the vector engine. The engine never panics on bad
/// input; every failure mode is a typed error.
#[derive(Debug, Error)]
pub enum VectorError {
    /// A vector had the wrong number of components for this index.
    #[error("dimension mismatch: index expects {expected}, got {got}")]
    InvalidDimension { expected: u32, got: usize },

    /// A vector was empty or otherwise unusable.
    #[error("invalid vector: {reason}")]
    InvalidVector { reason: String },

    /// Metadata failed validation (e.g. `kind` string too long).
    #[error("invalid metadata: {reason}")]
    InvalidMetadata { reason: String },

    /// `k` or other numeric argument out of range.
    #[error("invalid argument: {reason}")]
    InvalidArgument { reason: String },

    /// `create` was called on a directory that already contains an index.
    #[error("index already exists at {path}")]
    IndexAlreadyExists { path: String },

    /// `open` was called on a directory without a valid manifest.
    #[error("no index found at {path}")]
    IndexNotFound { path: String },

    /// On-disk format version is newer than this build understands.
    #[error("unsupported format version {found}, this build supports up to {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },

    /// File content failed magic / length / checksum validation.
    #[error("corrupt index file {file}: {reason}")]
    CorruptIndex { file: String, reason: String },

    /// Opened index configuration differs from the requested configuration.
    #[error("configuration mismatch: {reason}")]
    ConfigMismatch { reason: String },

    /// Operation attempted after `close()`.
    #[error("index is closed")]
    IndexClosed,

    /// Underlying filesystem failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl VectorError {
    pub(crate) fn invalid_vector(reason: impl Into<String>) -> Self {
        VectorError::InvalidVector {
            reason: reason.into(),
        }
    }

    pub(crate) fn invalid_argument(reason: impl Into<String>) -> Self {
        VectorError::InvalidArgument {
            reason: reason.into(),
        }
    }

    pub(crate) fn invalid_metadata(reason: impl Into<String>) -> Self {
        VectorError::InvalidMetadata {
            reason: reason.into(),
        }
    }

    pub(crate) fn corrupt(file: impl Into<String>, reason: impl Into<String>) -> Self {
        VectorError::CorruptIndex {
            file: file.into(),
            reason: reason.into(),
        }
    }
}

pub type Result<T> = std::result::Result<T, VectorError>;
