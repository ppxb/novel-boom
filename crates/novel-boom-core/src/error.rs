//! Shared error types for the core crate.

use std::path::PathBuf;

/// Convenient result alias used across core APIs.
pub type Result<T> = std::result::Result<T, Error>;

/// Domain and infrastructure errors surfaced by `novel-boom-core`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("config file not found: {0}")]
    ConfigNotFound(PathBuf),

    #[error("invalid config at {path}: {source}")]
    InvalidConfig {
        path: PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("unsupported download format: {0}")]
    UnsupportedFormat(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Message(String),
}
