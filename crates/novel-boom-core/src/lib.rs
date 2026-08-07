//! Business core for **novel-boom**.
//!
//! This crate has no UI dependencies. Presentation layers (TUI/CLI) should
//! depend only on the stable surface re-exported here:
//!
//! - [`config`] — TOML application settings
//! - [`model`] — pure domain types
//! - [`rule`] — book source packs (so-novel JSON)
//! - [`net`] — HTTP client
//! - [`service`] — use-cases (search, …)
//! - [`Error`] / [`Result`]

pub mod config;
pub mod error;
pub mod extract;
pub mod model;
pub mod net;
pub mod parse;
pub mod rule;
pub mod service;

pub use error::{Error, Result};

/// Crate version from Cargo metadata.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
