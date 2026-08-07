//! 界面绘制（纯展示，无业务逻辑）。

use novel_boom_core::config::Config;
use novel_boom_core::model::SearchHit;
use novel_boom_core::rule::SourceInfo;
use novel_boom_core::VERSION;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table, TableState,
    Wrap,
};
use ratatui::Frame;

use crate::screen::Screen;

/// 绘制时只读的应用快照。
pub struct UiModel<'a> {
    pub config: &'a Config,
    pub screen: &'a Screen,
    pub menu_state: &'a ListState,
    pub source_state: &'a TableState,
    pub sources: &'a [SourceInfo],
    pub searchable_sources: &'a [SourceInfo],
    pub search_pick_state: &'a TableState,
    pub search_result_state: &'a TableState,
    pub search_hits: &'a [SearchHit],
    pub search_input: &'a str,
    pub searching: bool,
    pub rules_path: &'a str,
    pub rules_error: Option<&'a str>,
    pub status: &'a str,
}

/// 主菜单：标题 + 说明。
pub fn menu_items() -> &'static [(&'static str, &'static str)] {
    &[
        ("聚合搜索", "在全部可搜索书源中查找书名或作者"),
        ("独立搜索", "仅在指定书源中搜索"),
        ("批量下载", "按书名列表批量下载（即将推出）"),
        ("书源一览", "查看当前规则包中的书源"),
        ("配置信息", "查看已加载的 TOML 配置"),
        ("退出程序", "结束 novel-boom"),
    ]
}

pub fn draw(frame: &mut Frame<'_>, model: &UiModel<'_>) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    draw_header(frame, chunks[0], model);
    match model.screen {
        Screen::Home => draw_home(frame, chunks[1], model),
        Screen::Config => draw_config(frame, chunks[1], model),
        Screen::Sources => draw_sources(frame, chunks[1], model),
        Screen::SingleSearchPickSource => draw_search_pick(frame, chunks[1], model),
        Screen::SingleSearchInput {
            source_id,
            source_name,
        } => draw_search_input(frame, chunks[1], model, *source_id, source_name),
        Screen::SingleSearchResults {
            source_id,
            source_name,
            keyword,
        } => draw_search_results(frame, chunks[1], model, *source_id, source_name, keyword),
        Screen::Placeholder(title) => draw_placeholder(frame, chunks[1], title),
    }
    draw_status(frame, chunks[2], model);

    if model.searching {
        draw_searching_overlay(frame, area);
    }
}

fn draw_header(frame: &mut Frame<'_>, area: Rect, model: &UiModel<'_>) {
    let title = Line::from(vec![
        Span::styled(
            " novel-boom ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(format!("v{VERSION}"), Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
        Span::styled(
            format!("规则={}", model.config.source.active_rules),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw("  "),
        Span::styled(
            format!("格式={}", model.config.download.format),
            Style::default().fg(Color::Green),
        ),
    ]);

    let paragraph = Paragraph::new(title).block(
        Block::default()
            .borders(Borders::ALL)
            .title("网络小说下载器"),
    );
    frame.render_widget(paragraph, area);
}

fn draw_home(frame: &mut Frame<'_>, area: Rect, model: &UiModel<'_>) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(area);

    let items: Vec<ListItem> = menu_items()
        .iter()
        .map(|(label, _)| {
            ListItem::new(Line::from(Span::styled(
                format!("  {label}"),
                Style::default().fg(Color::White),
            )))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("功能菜单"))
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    let mut state = model.menu_state.clone();
    frame.render_stateful_widget(list, chunks[0], &mut state);

    let selected = model.menu_state.selected().unwrap_or(0);
    let (label, blurb) = menu_items()[selected.min(menu_items().len() - 1)];

    let source_summary = if let Some(err) = model.rules_error {
        format!("加载失败：{err}")
    } else {
        format!(
            "共 {} 个 · 可搜索 {}",
            model.sources.len(),
            model.searchable_sources.len()
        )
    };

    let summary = vec![
        Line::from(Span::styled(
            label,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(blurb),
        Line::from(""),
        kv_line("下载路径", model.config.download.path.clone()),
        kv_line("激活规则", model.config.source.active_rules.clone()),
        kv_line("导出格式", model.config.download.format.to_string()),
        kv_line(
            "HTTP 代理",
            if model.config.proxy.enabled {
                format!(
                    "{}:{}（已启用）",
                    model.config.proxy.host, model.config.proxy.port
                )
            } else {
                "未启用".into()
            },
        ),
        kv_line("书源概况", source_summary),
        Line::from(""),
        Line::from(Span::styled(
            "操作：↑/↓ 或 j/k 移动 · Enter 确认 · q 退出",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let detail = Paragraph::new(summary)
        .block(Block::default().borders(Borders::ALL).title("概览"))
        .wrap(Wrap { trim: true });
    frame.render_widget(detail, chunks[1]);
}

fn draw_config(frame: &mut Frame<'_>, area: Rect, model: &UiModel<'_>) {
    let c = model.config;
    let lines = vec![
        section_line("应用"),
        kv_line("启动时检查更新", bool_zh(c.app.auto_update)),
        kv_line("GitHub 代理", empty_as_dash(&c.app.gh_proxy)),
        kv_line("CF 绕过地址", empty_as_dash(&c.app.cf_bypass)),
        Line::from(""),
        section_line("下载"),
        kv_line("路径", c.download.path.clone()),
        kv_line("格式", c.download.format.to_string()),
        kv_line("TXT 编码", c.download.txt_encoding.clone()),
        kv_line("保留章节缓存", bool_zh(c.download.preserve_chapter_cache)),
        Line::from(""),
        section_line("书源"),
        kv_line("语言", empty_as_dash(&c.source.language)),
        kv_line("激活规则", c.source.active_rules.clone()),
        kv_line(
            "指定书源 ID",
            if c.source.source_id == 0 {
                "未指定".into()
            } else {
                c.source.source_id.to_string()
            },
        ),
        kv_line(
            "搜索条数上限",
            if c.source.search_limit == 0 {
                "不限制".into()
            } else {
                c.source.search_limit.to_string()
            },
        ),
        kv_line("过滤低相关结果", bool_zh(c.source.search_filter)),
        Line::from(""),
        section_line("爬取"),
        kv_line(
            "并发",
            if c.crawl.concurrency == 0 {
                "自动".into()
            } else {
                c.crawl.concurrency.to_string()
            },
        ),
        kv_line(
            "请求间隔(ms)",
            format!("{} – {}", c.crawl.min_interval_ms, c.crawl.max_interval_ms),
        ),
        kv_line(
            "失败重试",
            format!(
                "{}（最多 {} 次）",
                bool_zh(c.crawl.retry),
                c.crawl.max_retries
            ),
        ),
        Line::from(""),
        section_line("代理"),
        kv_line(
            "HTTP 代理",
            if c.proxy.enabled {
                format!("{}:{}", c.proxy.host, c.proxy.port)
            } else {
                "未启用".into()
            },
        ),
    ];

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("配置信息（只读）"),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn draw_sources(frame: &mut Frame<'_>, area: Rect, model: &UiModel<'_>) {
    draw_source_table(
        frame,
        area,
        model,
        SourceTableView {
            sources: model.sources,
            state: model.source_state,
            title: "书源一览",
        },
    );
}

fn draw_search_pick(frame: &mut Frame<'_>, area: Rect, model: &UiModel<'_>) {
    draw_source_table(
        frame,
        area,
        model,
        SourceTableView {
            sources: model.searchable_sources,
            state: model.search_pick_state,
            title: "独立搜索 · 选择书源",
        },
    );
}

struct SourceTableView<'a> {
    sources: &'a [SourceInfo],
    state: &'a TableState,
    title: &'a str,
}

fn draw_source_table(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &UiModel<'_>,
    view: SourceTableView<'_>,
) {
    if let Some(err) = model.rules_error {
        let text = vec![
            Line::from(Span::styled(
                "无法加载书源规则",
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(err.to_string()),
            Line::from(""),
            Line::from("按 Esc 或 q 返回。"),
        ];
        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(view.title.to_string()),
            )
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(6)])
        .split(area);

    let header = Row::new(vec!["ID", "名称", "状态", "搜索", "代理", "站点"])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    let rows = view.sources.iter().map(|s| {
        let status = if s.disabled { "禁用" } else { "启用" };
        let search = if s.searchable { "是" } else { "否" };
        let proxy = if s.need_proxy { "需要" } else { "—" };
        let status_style = if s.disabled {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::Green)
        };
        Row::new(vec![
            Cell::from(s.id.to_string()),
            Cell::from(s.name.clone()),
            Cell::from(status).style(status_style),
            Cell::from(search),
            Cell::from(proxy),
            Cell::from(s.url.clone()),
        ])
    });

    let widths = [
        Constraint::Length(4),
        Constraint::Length(16),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Min(20),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(format!(
            "{}（{} · 共 {} 个）",
            view.title,
            model.config.source.active_rules,
            view.sources.len()
        )))
        .row_highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    let mut table_state = view.state.clone();
    frame.render_stateful_widget(table, chunks[0], &mut table_state);

    let detail = match view.state.selected().and_then(|i| view.sources.get(i)) {
        Some(source) => vec![
            Line::from(Span::styled(
                format!("[{}] {}", source.id, source.name),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            kv_line("网址", source.url.clone()),
            kv_line("备注", empty_as_dash(&source.comment)),
            kv_line("规则文件", model.rules_path.to_string()),
            Line::from(Span::styled(
                "↑/↓ 浏览 · Enter 选择 · Esc/q 返回",
                Style::default().fg(Color::DarkGray),
            )),
        ],
        None => vec![Line::from("未选择书源")],
    };

    let paragraph = Paragraph::new(detail)
        .block(Block::default().borders(Borders::ALL).title("书源详情"))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, chunks[1]);
}

fn draw_search_input(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &UiModel<'_>,
    source_id: u32,
    source_name: &str,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Min(3),
        ])
        .split(area);

    let info = Paragraph::new(vec![
        Line::from(Span::styled(
            "独立搜索",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("书源：[{source_id}] {source_name}")),
        Line::from("请输入书名或作者（尽量完整），回车开始搜索。"),
    ])
    .block(Block::default().borders(Borders::ALL).title("说明"));
    frame.render_widget(info, chunks[0]);

    let input = Paragraph::new(format!("» {}_", model.search_input)).block(
        Block::default()
            .borders(Borders::ALL)
            .title("关键词")
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(input, chunks[1]);

    let help = Paragraph::new(vec![
        Line::from("Enter 搜索 · Esc 返回书源列表 · Ctrl+u 清空输入"),
        Line::from(Span::styled(
            "提示：含 @js: / XPath 的书源暂不支持，会给出明确错误。",
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .block(Block::default().borders(Borders::ALL).title("操作"));
    frame.render_widget(help, chunks[2]);
}

fn draw_search_results(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &UiModel<'_>,
    source_id: u32,
    source_name: &str,
    keyword: &str,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(5), Constraint::Length(5)])
        .split(area);

    let head = Paragraph::new(vec![
        Line::from(format!(
            "书源 [{source_id}] {source_name} · 关键词「{keyword}」· 共 {} 条",
            model.search_hits.len()
        )),
        Line::from(Span::styled(
            "↑/↓ 浏览 · r 重新输入关键词 · Esc 返回主菜单",
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("独立搜索结果"),
    );
    frame.render_widget(head, chunks[0]);

    if model.search_hits.is_empty() {
        let empty = Paragraph::new("没有搜索到结果。可按 r 修改关键词后重试。")
            .block(Block::default().borders(Borders::ALL).title("列表"));
        frame.render_widget(empty, chunks[1]);
    } else {
        let header = Row::new(vec!["#", "书名", "作者", "最新章节", "更新"])
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            );

        let rows = model.search_hits.iter().enumerate().map(|(idx, hit)| {
            Row::new(vec![
                Cell::from((idx + 1).to_string()),
                Cell::from(hit.book_name.clone()),
                Cell::from(hit.author.clone()),
                Cell::from(truncate_chars(&hit.latest_chapter, 24)),
                Cell::from(truncate_chars(&hit.last_update_time, 16)),
            ])
        });

        let table = Table::new(
            rows,
            [
                Constraint::Length(4),
                Constraint::Percentage(28),
                Constraint::Percentage(18),
                Constraint::Percentage(32),
                Constraint::Percentage(18),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title("列表"))
        .row_highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

        let mut state = model.search_result_state.clone();
        frame.render_stateful_widget(table, chunks[1], &mut state);
    }

    let detail = match model
        .search_result_state
        .selected()
        .and_then(|i| model.search_hits.get(i))
    {
        Some(hit) => vec![
            Line::from(Span::styled(
                hit.book_name.clone(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            kv_line("作者", empty_as_dash(&hit.author)),
            kv_line("分类", empty_as_dash(&hit.category)),
            kv_line("状态", empty_as_dash(&hit.status)),
            kv_line("详情", hit.url.clone()),
        ],
        None => vec![Line::from("未选择结果")],
    };

    let paragraph = Paragraph::new(detail)
        .block(Block::default().borders(Borders::ALL).title("详情"))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, chunks[2]);
}

fn draw_searching_overlay(frame: &mut Frame<'_>, area: Rect) {
    let popup = centered_rect(50, 20, area);
    frame.render_widget(Clear, popup);
    let paragraph = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "正在搜索，请稍候…",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("网络请求进行中"),
    ])
    .alignment(ratatui::layout::Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("请稍候")
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(paragraph, popup);
}

fn draw_placeholder(frame: &mut Frame<'_>, area: Rect, title: &str) {
    let text = vec![
        Line::from(Span::styled(
            title.to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("该功能尚未实现，将在后续版本中提供。"),
        Line::from("按 Esc 或 q 返回主菜单。"),
    ];
    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("敬请期待"))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn draw_status(frame: &mut Frame<'_>, area: Rect, model: &UiModel<'_>) {
    let paragraph = Paragraph::new(Line::from(Span::raw(format!(" {} ", model.status)))).block(
        Block::default().borders(Borders::ALL).title("状态"),
    );
    frame.render_widget(paragraph, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn section_line(name: &str) -> Line<'static> {
    Line::from(Span::styled(
        format!("[{name}]"),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))
}

fn kv_line(key: &str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {key}："), Style::default().fg(Color::DarkGray)),
        Span::raw(value),
    ])
}

fn bool_zh(value: bool) -> String {
    if value { "是".into() } else { "否".into() }
}

fn empty_as_dash(value: &str) -> String {
    if value.is_empty() {
        "—".into()
    } else {
        value.to_string()
    }
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    let count = input.chars().count();
    if count <= max_chars {
        return input.to_string();
    }
    let mut s: String = input.chars().take(max_chars.saturating_sub(1)).collect();
    s.push('…');
    s
}
