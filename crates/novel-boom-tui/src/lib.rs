//! Terminal user interface for novel-boom.
//!
//! Depends on [`novel_boom_core`] for configuration and (later) use-cases.
//! This crate must not implement HTML parsing, HTTP crawling, or export logic.

mod app;
mod screen;

use novel_boom_core::config::Config;

/// Start the interactive TUI with an already-loaded configuration.
pub fn run(config: Config) -> anyhow::Result<()> {
    app::run(config)
}
