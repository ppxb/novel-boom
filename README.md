# novel-boom

Rust + TUI novel downloader. Inspired by [so-novel](https://github.com/freeok/so-novel), rebuilt for a single native binary, clearer module boundaries, and a real terminal UI.

> **Status:** Day 0 skeleton — config + TUI home shell. Search / crawl / export are not implemented yet.

## Principles

1. **Business modules separated from day one** — UI never owns parsing, HTTP, or export logic.
2. **Thin files, clear layers** — small modules, stable public API, easy to read and extend.
3. **TOML configuration** — modern app config; book source rules stay JSON for ecosystem compatibility.
4. **Prefer mature crates** — orchestrate well-known libraries instead of reinventing protocols and formats.

## Workspace layout

```text
novel-boom/
├── src/main.rs                 # thin binary: CLI flags → load config → TUI
├── crates/
│   ├── novel-boom-core/        # domain + config (no UI)
│   └── novel-boom-tui/         # ratatui presentation only
├── config.example.toml
├── rules/                      # so-novel-compatible source packs (JSON)
└── Cargo.toml                  # workspace
```

| Crate | Responsibility |
|-------|----------------|
| `novel-boom-core` | Config, models, (later) rules / parse / crawl / export / services |
| `novel-boom-tui` | Screens, input, rendering — calls core only |
| `novel-boom` (bin) | `clap` + tracing bootstrap |

## Quick start

```bash
cd novel-boom
cp config.example.toml config.toml
cargo run
```

Useful flags:

```bash
cargo run -- --help
cargo run -- --config path/to/config.toml
cargo run -- --print-config
```

Environment:

- `NOVEL_BOOM_CONFIG` — default config path (overridden by `--config`)
- `RUST_LOG` — tracing filter (default `info`)

## Configuration

Application settings use **TOML** (`config.toml`). See [`config.example.toml`](config.example.toml).

Book sources remain **JSON** under `rules/` (compatible with so-novel rule packs). Point `source.active_rules` at a file name such as `main.json`.

| so-novel (`config.ini`) | novel-boom (`config.toml`) |
|-------------------------|----------------------------|
| `[download] download-path` | `[download] path` |
| `[download] extname` | `[download] format` |
| `[source] active-rules` | `[source] active_rules` |
| `[crawl] min-interval` | `[crawl] min_interval_ms` |
| `[proxy] enabled/host/port` | `[proxy] enabled/host/port` |

## 终端界面（当前）

界面文案为**中文**。

- 主菜单：`↑/↓` 或 `j/k` 移动，`Enter` 确认，`q` 退出
- **独立搜索**：选书源 → 输入关键词 → 结果列表
- **打开目录**：搜索结果 Enter → 详情 + 目录 → `1` 全本 / `2` 范围 / `3` 最新 N 章
- **书源一览** / **配置信息**
- 聚合搜索 / 批量下载 / 正文下载：占位（后续里程碑）
- 规则：CSS；暂不支持 `@js:` / XPath

```bash
cargo run -- --print-sources
cargo run -- --search "诡秘之主" --source-id 1
cargo run -- --toc "http://example.com/book/1/" --source-id 1
```

## Roadmap (short)

1. ~~Workspace + TOML config + TUI home~~ ← you are here
2. Rule catalog load + source list screen
3. CSS extract + single-source search (fixture-tested)
4. TOC / chapter / concurrent crawl + TXT export
5. Aggregated search + EPUB + optional JS (`rquickjs`)

## Develop

```bash
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 免责声明

本项目仅供学习、研究和技术交流使用。项目作者与任何第三方服务、原始应用或内容提供方无关。 使用者应自行遵守当地法律法规以及相关服务条款。因使用本项目产生的任何法律、版权、账号、数据或财务风险均由使用者自行承担。
