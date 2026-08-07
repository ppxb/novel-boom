//! novel-boom 程序入口。
//!
//! 职责尽量薄：解析参数 → 加载配置/书源 → 交给 TUI 或 CLI 搜索。

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use novel_boom_core::config::{self, Config};
use novel_boom_core::net::HttpClient;
use novel_boom_core::rule::RuleCatalog;
use novel_boom_core::service;
use novel_boom_tui::TuiOptions;
use tokio::runtime::Runtime;
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

    /// 命令行独立搜索关键词（不进入 TUI）
    #[arg(long, value_name = "KEYWORD")]
    search: Option<String>,

    /// 独立搜索使用的书源 ID（配合 --search；默认读配置 source_id）
    #[arg(long, value_name = "ID")]
    source_id: Option<u32>,
}

fn main() -> ExitCode {
    init_tracing();

    let cli = Cli::parse();
    let path = config::resolve_config_path(cli.config.as_deref());

    let mut cfg = match config::load_or_default(&path) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("错误：无法加载配置 {}：{err}", path.display());
            return ExitCode::FAILURE;
        }
    };

    if let Some(id) = cli.source_id {
        cfg.source.source_id = id;
    }

    if cli.print_config {
        return match print_config(&cfg) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("错误：{err}");
                ExitCode::FAILURE
            }
        };
    }

    let catalog = match RuleCatalog::load(&cfg, Some(&path)) {
        Ok(catalog) => catalog,
        Err(err) => {
            if cli.print_sources || cli.search.is_some() {
                eprintln!("错误：无法加载书源：{err}");
                return ExitCode::FAILURE;
            }
            tracing::warn!(error = %err, "书源规则加载失败，界面仍可启动");
            return run_tui(cfg, None, Some(err.to_string()));
        }
    };

    if cli.print_sources {
        print_sources(&catalog);
        return ExitCode::SUCCESS;
    }

    if let Some(keyword) = cli.search.as_deref() {
        return run_cli_search(&cfg, &catalog, keyword);
    }

    tracing::info!(
        path = %catalog.path.display(),
        count = catalog.len(),
        searchable = catalog.searchable_count(),
        "书源规则已加载"
    );

    run_tui(cfg, Some(catalog), None)
}

fn run_tui(cfg: Config, catalog: Option<RuleCatalog>, rules_error: Option<String>) -> ExitCode {
    let http = match HttpClient::from_config(&cfg) {
        Ok(http) => http,
        Err(err) => {
            eprintln!("错误：创建 HTTP 客户端失败：{err}");
            return ExitCode::FAILURE;
        }
    };

    let runtime = match Runtime::new() {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("错误：创建异步运行时失败：{err}");
            return ExitCode::FAILURE;
        }
    };

    tracing::info!(
        version = novel_boom_core::VERSION,
        rules = %cfg.source.active_rules,
        "启动终端界面"
    );

    let options = TuiOptions {
        config: cfg,
        catalog,
        rules_error,
        http,
        runtime,
    };

    if let Err(err) = novel_boom_tui::run(options) {
        eprintln!("错误：{err:#}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn run_cli_search(cfg: &Config, catalog: &RuleCatalog, keyword: &str) -> ExitCode {
    let source_id = cfg.source.source_id;
    if source_id == 0 {
        eprintln!("错误：请通过 --source-id 或配置 [source].source_id 指定书源");
        return ExitCode::FAILURE;
    }

    let http = match HttpClient::from_config(cfg) {
        Ok(http) => http,
        Err(err) => {
            eprintln!("错误：创建 HTTP 客户端失败：{err}");
            return ExitCode::FAILURE;
        }
    };

    let runtime = match Runtime::new() {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("错误：创建异步运行时失败：{err}");
            return ExitCode::FAILURE;
        }
    };

    println!("正在搜索：书源 {source_id} · 关键词「{keyword}」…");

    match runtime.block_on(service::single_search(
        &http, catalog, cfg, source_id, keyword,
    )) {
        Ok(hits) => {
            if hits.is_empty() {
                println!("无结果");
                return ExitCode::SUCCESS;
            }
            println!("共 {} 条：", hits.len());
            for (idx, hit) in hits.iter().enumerate() {
                println!(
                    "{:>3}. {} · {} · {} · {}",
                    idx + 1,
                    hit.book_name,
                    empty_dash(&hit.author),
                    empty_dash(&hit.latest_chapter),
                    hit.url
                );
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("错误：搜索失败：{err}");
            ExitCode::FAILURE
        }
    }
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

fn empty_dash(value: &str) -> &str {
    if value.is_empty() { "—" } else { value }
}
