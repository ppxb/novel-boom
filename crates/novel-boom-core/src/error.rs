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

    #[error("rules file not found: {0}")]
    RulesNotFound(PathBuf),

    #[error("invalid rules at {path}: {source}")]
    InvalidRules {
        path: PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("book source not found: id={0}")]
    SourceNotFound(u32),

    #[error("book source is disabled: {0}")]
    SourceDisabled(String),

    #[error("book source does not support search: {0}")]
    SearchNotSupported(String),

    #[error("feature not supported yet: {0}")]
    UnsupportedFeature(String),

    #[error("invalid selector `{selector}`: {message}")]
    InvalidSelector { selector: String, message: String },

    #[error("{0}")]
    Http(String),

    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Message(String),
}

impl From<url::ParseError> for Error {
    fn from(value: url::ParseError) -> Self {
        Error::InvalidUrl(value.to_string())
    }
}
