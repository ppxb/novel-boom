//! Application state and the main event loop.

use std::io::{self, Stdout};
use std::time::Duration;

use anyhow::Context;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use novel_boom_core::config::Config;
use novel_boom_core::VERSION;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use crate::screen::Screen;

/// Runs the TUI until the user quits.
pub fn run(config: Config) -> anyhow::Result<()> {
    let mut terminal = setup_terminal().context("failed to initialize terminal")?;
    let mut app = App::new(config);

    let result = app_loop(&mut terminal, &mut app);

    restore_terminal(&mut terminal).context("failed to restore terminal")?;
    result
}

struct App {
    config: Config,
    screen: Screen,
    menu_state: ListState,
    should_quit: bool,
    status: String,
}

impl App {
    fn new(config: Config) -> Self {
        let mut menu_state = ListState::default();
        menu_state.select(Some(0));
        Self {
            config,
            screen: Screen::Home,
            menu_state,
            should_quit: false,
            status: "Ready — ↑/↓ move · Enter open · q quit".into(),
        }
    }

    fn menu_items() -> &'static [(&'static str, &'static str)] {
        &[
            ("Aggregated Search", "Search across all enabled sources"),
            ("Single-source Search", "Search within one book source"),
            ("Batch Download", "Download from a list of URLs (soon)"),
            ("Source Catalog", "Browse active rule pack sources"),
            ("Configuration", "View loaded TOML settings"),
            ("Quit", "Exit novel-boom"),
        ]
    }

    fn on_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match self.screen {
            Screen::Home => self.on_home_key(key),
            Screen::Config => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
                    self.screen = Screen::Home;
                    self.status = "Back to home".into();
                }
            }
            Screen::Placeholder(title) => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
                    let _ = title;
                    self.screen = Screen::Home;
                    self.status = "Back to home".into();
                }
            }
        }
    }

    fn on_home_key(&mut self, key: KeyEvent) {
        let len = Self::menu_items().len();
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let i = self.menu_state.selected().unwrap_or(0);
                self.menu_state.select(Some(if i == 0 { len - 1 } else { i - 1 }));
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let i = self.menu_state.selected().unwrap_or(0);
                self.menu_state.select(Some(if i + 1 >= len { 0 } else { i + 1 }));
            }
            KeyCode::Enter => self.activate_menu(),
            _ => {}
        }
    }

    fn activate_menu(&mut self) {
        let Some(i) = self.menu_state.selected() else {
            return;
        };
        match i {
            0 => {
                self.screen = Screen::Placeholder("Aggregated Search");
                self.status = "Not implemented yet — Day 0 skeleton".into();
            }
            1 => {
                self.screen = Screen::Placeholder("Single-source Search");
                self.status = "Not implemented yet — Day 0 skeleton".into();
            }
            2 => {
                self.screen = Screen::Placeholder("Batch Download");
                self.status = "Not implemented yet — Day 0 skeleton".into();
            }
            3 => {
                self.screen = Screen::Placeholder("Source Catalog");
                self.status = "Not implemented yet — Day 0 skeleton".into();
            }
            4 => {
                self.screen = Screen::Config;
                self.status = "Esc / q to go back".into();
            }
            5 => self.should_quit = true,
            _ => {}
        }
    }
}

fn app_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> anyhow::Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| draw(frame, app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.on_key(key);
            }
        }
    }
    Ok(())
}

fn draw(frame: &mut ratatui::Frame<'_>, app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    draw_header(frame, chunks[0], app);
    match app.screen {
        Screen::Home => draw_home(frame, chunks[1], app),
        Screen::Config => draw_config(frame, chunks[1], app),
        Screen::Placeholder(title) => draw_placeholder(frame, chunks[1], title),
    }
    draw_status(frame, chunks[2], app);
}

fn draw_header(frame: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let title = Line::from(vec![
        Span::styled(
            " novel-boom ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("v{VERSION}"),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw("  "),
        Span::styled(
            format!("rules={}", app.config.source.active_rules),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw("  "),
        Span::styled(
            format!("format={}", app.config.download.format),
            Style::default().fg(Color::Green),
        ),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Novel Boom");
    let paragraph = Paragraph::new(title).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_home(frame: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    let items: Vec<ListItem> = App::menu_items()
        .iter()
        .map(|(label, _)| {
            ListItem::new(Line::from(Span::styled(
                format!("  {label}"),
                Style::default().fg(Color::White),
            )))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Menu"),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    let mut state = app.menu_state.clone();
    frame.render_stateful_widget(list, chunks[0], &mut state);

    let selected = app.menu_state.selected().unwrap_or(0);
    let (label, blurb) = App::menu_items()[selected];
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
        Line::from(vec![
            Span::styled("Download path: ", Style::default().fg(Color::DarkGray)),
            Span::raw(app.config.download.path.clone()),
        ]),
        Line::from(vec![
            Span::styled("Active rules:  ", Style::default().fg(Color::DarkGray)),
            Span::raw(app.config.source.active_rules.clone()),
        ]),
        Line::from(vec![
            Span::styled("Export format: ", Style::default().fg(Color::DarkGray)),
            Span::raw(app.config.download.format.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Proxy:         ", Style::default().fg(Color::DarkGray)),
            Span::raw(if app.config.proxy.enabled {
                format!(
                    "{}:{} (on)",
                    app.config.proxy.host, app.config.proxy.port
                )
            } else {
                "off".into()
            }),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Day 0: skeleton only — crawl/search land next.",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let detail = Paragraph::new(summary)
        .block(Block::default().borders(Borders::ALL).title("Overview"))
        .wrap(Wrap { trim: true });
    frame.render_widget(detail, chunks[1]);
}

fn draw_config(frame: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let c = &app.config;
    let lines = vec![
        section("app"),
        kv("auto_update", c.app.auto_update.to_string()),
        kv("gh_proxy", empty_as_dash(&c.app.gh_proxy)),
        kv("cf_bypass", empty_as_dash(&c.app.cf_bypass)),
        Line::from(""),
        section("download"),
        kv("path", c.download.path.clone()),
        kv("format", c.download.format.to_string()),
        kv("txt_encoding", c.download.txt_encoding.clone()),
        kv(
            "preserve_chapter_cache",
            c.download.preserve_chapter_cache.to_string(),
        ),
        Line::from(""),
        section("source"),
        kv("language", empty_as_dash(&c.source.language)),
        kv("active_rules", c.source.active_rules.clone()),
        kv("source_id", c.source.source_id.to_string()),
        kv("search_limit", c.source.search_limit.to_string()),
        kv("search_filter", c.source.search_filter.to_string()),
        Line::from(""),
        section("crawl"),
        kv("concurrency", c.crawl.concurrency.to_string()),
        kv(
            "interval_ms",
            format!(
                "{} – {}",
                c.crawl.min_interval_ms, c.crawl.max_interval_ms
            ),
        ),
        kv("retry", format!("{} (max {})", c.crawl.retry, c.crawl.max_retries)),
        Line::from(""),
        section("proxy"),
        kv(
            "proxy",
            if c.proxy.enabled {
                format!("{}:{}", c.proxy.host, c.proxy.port)
            } else {
                "disabled".into()
            },
        ),
    ];

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Configuration (read-only)"),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn draw_placeholder(frame: &mut ratatui::Frame<'_>, area: Rect, title: &str) {
    let text = vec![
        Line::from(Span::styled(
            title,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("This screen is reserved for a later milestone."),
        Line::from("Press Esc or q to return to the home menu."),
    ];
    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Coming soon"))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn draw_status(frame: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let paragraph = Paragraph::new(Line::from(Span::raw(format!(" {} ", app.status)))).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Status"),
    );
    frame.render_widget(paragraph, area);
}

fn section(name: &str) -> Line<'static> {
    Line::from(Span::styled(
        format!("[{name}]"),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))
}

fn kv(key: &str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {key}: "), Style::default().fg(Color::DarkGray)),
        Span::raw(value),
    ])
}

fn empty_as_dash(value: &str) -> String {
    if value.is_empty() {
        "—".into()
    } else {
        value.to_string()
    }
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout))
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
