//! 应用状态与主事件循环。

use std::io::{self, Stdout};
use std::time::Duration;

use anyhow::Context;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use novel_boom_core::config::Config;
use novel_boom_core::rule::{RuleCatalog, SourceInfo};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::{ListState, TableState};

use crate::screen::Screen;
use crate::ui::{self, UiModel};

/// TUI 启动参数。
pub struct TuiOptions {
    pub config: Config,
    /// 成功加载的书源目录；失败时为 `None`，界面展示错误信息。
    pub catalog: Option<RuleCatalog>,
    /// 加载失败时的说明（中文）。
    pub rules_error: Option<String>,
}

/// 运行 TUI，直到用户退出。
pub fn run(options: TuiOptions) -> anyhow::Result<()> {
    let mut terminal = setup_terminal().context("初始化终端失败")?;
    let mut app = App::new(options);

    let result = app_loop(&mut terminal, &mut app);

    restore_terminal(&mut terminal).context("恢复终端失败")?;
    result
}

struct App {
    config: Config,
    screen: Screen,
    menu_state: ListState,
    source_state: TableState,
    sources: Vec<SourceInfo>,
    rules_path: String,
    rules_error: Option<String>,
    should_quit: bool,
    status: String,
}

impl App {
    fn new(options: TuiOptions) -> Self {
        let mut menu_state = ListState::default();
        menu_state.select(Some(0));

        let mut source_state = TableState::default();

        let (sources, rules_path, status) = match options.catalog {
            Some(catalog) => {
                let path = catalog.path.display().to_string();
                let sources = catalog.sources();
                if !sources.is_empty() {
                    source_state.select(Some(0));
                }
                let status = format!(
                    "就绪 — 已加载 {} 个书源 · ↑/↓ 移动 · Enter 打开 · q 退出",
                    sources.len()
                );
                (sources, path, status)
            }
            None => {
                let status = match &options.rules_error {
                    Some(err) => format!("就绪 — 书源未加载：{err}"),
                    None => "就绪 — ↑/↓ 移动 · Enter 打开 · q 退出".into(),
                };
                (Vec::new(), String::new(), status)
            }
        };

        Self {
            config: options.config,
            screen: Screen::Home,
            menu_state,
            source_state,
            sources,
            rules_path,
            rules_error: options.rules_error,
            should_quit: false,
            status,
        }
    }

    fn on_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match self.screen {
            Screen::Home => self.on_home_key(key),
            Screen::Config | Screen::Placeholder(_) => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
                    self.back_home("已返回主菜单");
                }
            }
            Screen::Sources => self.on_sources_key(key),
        }
    }

    fn on_home_key(&mut self, key: KeyEvent) {
        let len = ui::menu_items().len();
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let i = self.menu_state.selected().unwrap_or(0);
                self.menu_state
                    .select(Some(if i == 0 { len - 1 } else { i - 1 }));
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let i = self.menu_state.selected().unwrap_or(0);
                self.menu_state
                    .select(Some(if i + 1 >= len { 0 } else { i + 1 }));
            }
            KeyCode::Enter => self.activate_menu(),
            _ => {}
        }
    }

    fn on_sources_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.back_home("已返回主菜单");
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.sources.is_empty() {
                    return;
                }
                let len = self.sources.len();
                let i = self.source_state.selected().unwrap_or(0);
                self.source_state
                    .select(Some(if i == 0 { len - 1 } else { i - 1 }));
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.sources.is_empty() {
                    return;
                }
                let len = self.sources.len();
                let i = self.source_state.selected().unwrap_or(0);
                self.source_state
                    .select(Some(if i + 1 >= len { 0 } else { i + 1 }));
            }
            _ => {}
        }
    }

    fn activate_menu(&mut self) {
        let Some(i) = self.menu_state.selected() else {
            return;
        };
        match i {
            0 => {
                self.screen = Screen::Placeholder("聚合搜索");
                self.status = "功能尚未实现".into();
            }
            1 => {
                self.screen = Screen::Placeholder("独立搜索");
                self.status = "功能尚未实现".into();
            }
            2 => {
                self.screen = Screen::Placeholder("批量下载");
                self.status = "功能尚未实现".into();
            }
            3 => {
                self.screen = Screen::Sources;
                self.status = if self.rules_error.is_some() {
                    "书源规则加载失败，请检查配置".into()
                } else {
                    format!(
                        "书源一览 — 共 {} 个 · ↑/↓ 浏览 · Esc 返回",
                        self.sources.len()
                    )
                };
            }
            4 => {
                self.screen = Screen::Config;
                self.status = "配置信息（只读）· Esc / q 返回".into();
            }
            5 => self.should_quit = true,
            _ => {}
        }
    }

    fn back_home(&mut self, status: &str) {
        self.screen = Screen::Home;
        self.status = status.into();
    }

    fn ui_model(&self) -> UiModel<'_> {
        UiModel {
            config: &self.config,
            screen: self.screen,
            menu_state: &self.menu_state,
            source_state: &self.source_state,
            sources: &self.sources,
            rules_path: &self.rules_path,
            rules_error: self.rules_error.as_deref(),
            status: &self.status,
        }
    }
}

fn app_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> anyhow::Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| ui::draw(frame, &app.ui_model()))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.on_key(key);
            }
        }
    }
    Ok(())
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
