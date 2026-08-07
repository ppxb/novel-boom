//! Business core for **novel-boom**.
//!
//! This crate has no UI dependencies. Presentation layers (TUI/CLI) should
//! depend only on the stable surface re-exported here:
//!
//! - [`config`] — TOML application settings
//! - [`model`] — pure domain types
//! - [`rule`] — book source packs (so-novel JSON)
//! - [`Error`] / [`Result`]
//!
//! Crawl, parse, export, and use-case services will land in later milestones
//! without changing this boundary idea.

pub mod config;
pub mod error;
pub mod model;
pub mod rule;

pub use error::{Error, Result};

/// Crate version from Cargo metadata.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
