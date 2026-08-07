//! novel-boom binary entrypoint.
//!
//! Thin wiring only: parse CLI flags, load config, hand off to the TUI.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use novel_boom_core::config::{self, Config};
use tracing_subscriber::EnvFilter;

/// Novel downloader with a terminal UI.
#[derive(Debug, Parser)]
#[command(
    name = "novel-boom",
    version,
    about = "Download web novels via a terminal UI",
    long_about = None
)]
struct Cli {
    /// Path to config.toml (overrides NOVEL_BOOM_CONFIG and ./config.toml)
    #[arg(short, long, value_name = "PATH", env = "NOVEL_BOOM_CONFIG")]
    config: Option<PathBuf>,

    /// Print the resolved configuration as TOML and exit
    #[arg(long)]
    print_config: bool,
}

fn main() -> ExitCode {
    init_tracing();

    let cli = Cli::parse();
    let path = config::resolve_config_path(cli.config.as_deref());

    let loaded = config::load_or_default(&path);
    let cfg = match loaded {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("error: failed to load config from {}: {err}", path.display());
            return ExitCode::FAILURE;
        }
    };

    if cli.print_config {
        match print_config(&cfg) {
            Ok(()) => return ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("error: {err}");
                return ExitCode::FAILURE;
            }
        }
    }

    tracing::info!(
        version = novel_boom_core::VERSION,
        config = %path.display(),
        rules = %cfg.source.active_rules,
        "starting TUI"
    );

    if let Err(err) = novel_boom_tui::run(cfg) {
        eprintln!("error: {err:#}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}

fn print_config(cfg: &Config) -> anyhow::Result<()> {
    let rendered = toml::to_string_pretty(cfg)?;
    print!("{rendered}");
    Ok(())
}
