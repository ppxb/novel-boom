//! novel-boom 程序入口。
//!
//! 职责尽量薄：解析参数 → 加载配置/书源 → 交给 TUI。

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use novel_boom_core::config::{self, Config};
use novel_boom_core::rule::RuleCatalog;
use novel_boom_tui::TuiOptions;
use tracing_subscriber::EnvFilter;

/// 带终端界面的网络小说下载器。
#[derive(Debug, Parser)]
#[command(
    name = "novel-boom",
    version,
    about = "网络小说下载器（终端界面）",
    long_about = None
)]
struct Cli {
    /// 配置文件路径（覆盖 NOVEL_BOOM_CONFIG 与 ./config.toml）
    #[arg(short, long, value_name = "PATH", env = "NOVEL_BOOM_CONFIG")]
    config: Option<PathBuf>,

    /// 打印解析后的配置（TOML）并退出
    #[arg(long)]
    print_config: bool,

    /// 打印当前激活书源列表并退出
    #[arg(long)]
    print_sources: bool,
}

fn main() -> ExitCode {
    init_tracing();

    let cli = Cli::parse();
    let path = config::resolve_config_path(cli.config.as_deref());

    let cfg = match config::load_or_default(&path) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("错误：无法加载配置 {}：{err}", path.display());
            return ExitCode::FAILURE;
        }
    };

    if cli.print_config {
        return match print_config(&cfg) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("错误：{err}");
                ExitCode::FAILURE
            }
        };
    }

    let catalog_result = RuleCatalog::load(&cfg, Some(&path));

    if cli.print_sources {
        return match catalog_result {
            Ok(catalog) => {
                print_sources(&catalog);
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("错误：无法加载书源：{err}");
                ExitCode::FAILURE
            }
        };
    }

    let (catalog, rules_error) = match catalog_result {
        Ok(catalog) => {
            tracing::info!(
                path = %catalog.path.display(),
                count = catalog.len(),
                searchable = catalog.searchable_count(),
                "书源规则已加载"
            );
            (Some(catalog), None)
        }
        Err(err) => {
            tracing::warn!(error = %err, "书源规则加载失败，界面仍可启动");
            (None, Some(err.to_string()))
        }
    };

    tracing::info!(
        version = novel_boom_core::VERSION,
        config = %path.display(),
        rules = %cfg.source.active_rules,
        "启动终端界面"
    );

    let options = TuiOptions {
        config: cfg,
        catalog,
        rules_error,
    };

    if let Err(err) = novel_boom_tui::run(options) {
        eprintln!("错误：{err:#}");
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

fn print_sources(catalog: &RuleCatalog) {
    println!("规则文件: {}", catalog.path.display());
    println!(
        "共 {} 个书源（启用 {} · 可搜索 {}）",
        catalog.len(),
        catalog.enabled_count(),
        catalog.searchable_count()
    );
    println!();
    println!("ID   名称             状态   搜索   网址");
    for source in catalog.sources() {
        let status = if source.disabled { "禁用" } else { "启用" };
        let searchable = if source.searchable { "是" } else { "否" };
        println!(
            "{:<4} {:<16} {:<6} {:<6} {}",
            source.id,
            truncate_display(&source.name, 16),
            status,
            searchable,
            source.url
        );
    }
}

fn truncate_display(input: &str, max_chars: usize) -> String {
    let count = input.chars().count();
    if count <= max_chars {
        return input.to_string();
    }
    let mut s: String = input.chars().take(max_chars.saturating_sub(1)).collect();
    s.push('…');
    s
}
