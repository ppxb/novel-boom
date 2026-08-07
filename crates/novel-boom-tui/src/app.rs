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
use novel_boom_core::model::SearchHit;
use novel_boom_core::net::HttpClient;
use novel_boom_core::rule::{RuleCatalog, SourceInfo};
use novel_boom_core::service;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::{ListState, TableState};
use tokio::runtime::Runtime;

use crate::screen::Screen;
use crate::ui::{self, UiModel};

/// TUI 启动参数。
pub struct TuiOptions {
    pub config: Config,
    /// 成功加载的书源目录；失败时为 `None`，界面展示错误信息。
    pub catalog: Option<RuleCatalog>,
    /// 加载失败时的说明（中文）。
    pub rules_error: Option<String>,
    pub http: HttpClient,
    pub runtime: Runtime,
}

/// 运行 TUI，直到用户退出。
pub fn run(options: TuiOptions) -> anyhow::Result<()> {
    let mut terminal = setup_terminal().context("初始化终端失败")?;
    let mut app = App::new(options);

    let result = app_loop(&mut terminal, &mut app);

    restore_terminal(&mut terminal).context("恢复终端失败")?;
    result
}

fn redraw(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &App) -> anyhow::Result<()> {
    terminal.draw(|frame| ui::draw(frame, &app.ui_model()))?;
    Ok(())
}

struct App {
    config: Config,
    catalog: Option<RuleCatalog>,
    http: HttpClient,
    runtime: Runtime,
    screen: Screen,
    menu_state: ListState,
    source_state: TableState,
    search_pick_state: TableState,
    search_result_state: TableState,
    sources: Vec<SourceInfo>,
    searchable_sources: Vec<SourceInfo>,
    search_input: String,
    search_hits: Vec<SearchHit>,
    searching: bool,
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
        let mut search_pick_state = TableState::default();

        let (sources, searchable_sources, rules_path, status) = match &options.catalog {
            Some(catalog) => {
                let path = catalog.path.display().to_string();
                let sources = catalog.sources();
                let searchable_sources = catalog.searchable_sources();
                if !sources.is_empty() {
                    source_state.select(Some(0));
                }
                if !searchable_sources.is_empty() {
                    search_pick_state.select(Some(0));
                }
                let status = format!(
                    "就绪 — 已加载 {} 个书源（可搜索 {}）· ↑/↓ 移动 · Enter 打开 · q 退出",
                    sources.len(),
                    searchable_sources.len()
                );
                (sources, searchable_sources, path, status)
            }
            None => {
                let status = match &options.rules_error {
                    Some(err) => format!("就绪 — 书源未加载：{err}"),
                    None => "就绪 — ↑/↓ 移动 · Enter 打开 · q 退出".into(),
                };
                (Vec::new(), Vec::new(), String::new(), status)
            }
        };

        Self {
            config: options.config,
            catalog: options.catalog,
            http: options.http,
            runtime: options.runtime,
            screen: Screen::Home,
            menu_state,
            source_state,
            search_pick_state,
            search_result_state: TableState::default(),
            sources,
            searchable_sources,
            search_input: String::new(),
            search_hits: Vec::new(),
            searching: false,
            rules_path,
            rules_error: options.rules_error,
            should_quit: false,
            status,
        }
    }

    fn on_key(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> anyhow::Result<()> {
        if key.kind != KeyEventKind::Press || self.searching {
            return Ok(());
        }

        match &self.screen {
            Screen::Home => self.on_home_key(key),
            Screen::Config | Screen::Placeholder(_) => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
                    self.back_home("已返回主菜单");
                }
            }
            Screen::Sources => self.on_sources_key(key),
            Screen::SingleSearchPickSource => self.on_search_pick_key(key),
            Screen::SingleSearchInput { .. } => self.on_search_input_key(key, terminal)?,
            Screen::SingleSearchResults { .. } => self.on_search_results_key(key),
        }
        Ok(())
    }

    fn on_home_key(&mut self, key: KeyEvent) {
        let len = ui::menu_items().len();
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
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
            KeyCode::Esc | KeyCode::Char('q') => self.back_home("已返回主菜单"),
            KeyCode::Up | KeyCode::Char('k') => {
                move_table_sel(&mut self.source_state, self.sources.len(), -1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                move_table_sel(&mut self.source_state, self.sources.len(), 1);
            }
            _ => {}
        }
    }

    fn on_search_pick_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.back_home("已返回主菜单"),
            KeyCode::Up | KeyCode::Char('k') => {
                move_table_sel(
                    &mut self.search_pick_state,
                    self.searchable_sources.len(),
                    -1,
                );
            }
            KeyCode::Down | KeyCode::Char('j') => {
                move_table_sel(
                    &mut self.search_pick_state,
                    self.searchable_sources.len(),
                    1,
                );
            }
            KeyCode::Enter => {
                if let Some(source) = self
                    .search_pick_state
                    .selected()
                    .and_then(|i| self.searchable_sources.get(i))
                    .cloned()
                {
                    self.open_search_input(source.id, source.name);
                }
            }
            _ => {}
        }
    }

    fn on_search_input_key(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> anyhow::Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.screen = Screen::SingleSearchPickSource;
                self.status = "已返回书源选择".into();
            }
            KeyCode::Enter => self.run_search_from_input(terminal)?,
            KeyCode::Backspace => {
                self.search_input.pop();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.search_input.clear();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.search_input.push(c);
            }
            _ => {}
        }
        Ok(())
    }

    fn on_search_results_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.back_home("已返回主菜单"),
            KeyCode::Char('r') => {
                if let Screen::SingleSearchResults {
                    source_id,
                    source_name,
                    ..
                } = &self.screen
                {
                    let id = *source_id;
                    let name = source_name.clone();
                    self.open_search_input(id, name);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                move_table_sel(&mut self.search_result_state, self.search_hits.len(), -1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                move_table_sel(&mut self.search_result_state, self.search_hits.len(), 1);
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
            1 => self.open_single_search(),
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

    fn open_single_search(&mut self) {
        if self.catalog.is_none() {
            self.status = "无法搜索：书源规则未加载".into();
            return;
        }
        if self.searchable_sources.is_empty() {
            self.status = "当前规则包没有可搜索书源".into();
            return;
        }

        // 配置里指定了 source_id 时，直接进入输入页。
        if self.config.source.source_id > 0 {
            if let Some(source) = self
                .searchable_sources
                .iter()
                .find(|s| s.id == self.config.source.source_id)
                .cloned()
            {
                self.open_search_input(source.id, source.name);
                return;
            }
            self.status = format!(
                "配置的 source_id={} 不可搜索，请手动选择书源",
                self.config.source.source_id
            );
        }

        if self.search_pick_state.selected().is_none() && !self.searchable_sources.is_empty() {
            self.search_pick_state.select(Some(0));
        }
        self.screen = Screen::SingleSearchPickSource;
        self.status = format!(
            "独立搜索 — 选择书源（可搜索 {} 个）· Enter 确认",
            self.searchable_sources.len()
        );
    }

    fn open_search_input(&mut self, source_id: u32, source_name: String) {
        self.search_input.clear();
        self.search_hits.clear();
        self.search_result_state.select(None);
        self.status = format!("已选择 [{source_id}] {source_name}，请输入关键词");
        self.screen = Screen::SingleSearchInput {
            source_id,
            source_name,
        };
    }

    fn run_search_from_input(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> anyhow::Result<()> {
        let Screen::SingleSearchInput {
            source_id,
            source_name,
        } = &self.screen
        else {
            return Ok(());
        };
        let source_id = *source_id;
        let source_name = source_name.clone();
        let keyword = self.search_input.trim().to_string();
        if keyword.is_empty() {
            self.status = "关键词不能为空".into();
            return Ok(());
        }

        let Some(catalog) = self.catalog.as_ref() else {
            self.status = "书源未加载".into();
            return Ok(());
        };

        self.searching = true;
        self.status = format!("正在搜索「{keyword}」…");
        redraw(terminal, self)?;

        let result = self.runtime.block_on(service::single_search(
            &self.http,
            catalog,
            &self.config,
            source_id,
            &keyword,
        ));

        self.searching = false;

        match result {
            Ok(hits) => {
                let count = hits.len();
                self.search_hits = hits;
                if count > 0 {
                    self.search_result_state.select(Some(0));
                } else {
                    self.search_result_state.select(None);
                }
                self.screen = Screen::SingleSearchResults {
                    source_id,
                    source_name: source_name.clone(),
                    keyword: keyword.clone(),
                };
                self.status = format!(
                    "搜索完成 — [{source_id}] {source_name} · 「{keyword}」· {count} 条 · r 重搜"
                );
            }
            Err(err) => {
                self.status = format!("搜索失败：{err}");
            }
        }
        Ok(())
    }

    fn back_home(&mut self, status: &str) {
        self.screen = Screen::Home;
        self.status = status.into();
    }

    fn ui_model(&self) -> UiModel<'_> {
        UiModel {
            config: &self.config,
            screen: &self.screen,
            menu_state: &self.menu_state,
            source_state: &self.source_state,
            sources: &self.sources,
            searchable_sources: &self.searchable_sources,
            search_pick_state: &self.search_pick_state,
            search_result_state: &self.search_result_state,
            search_hits: &self.search_hits,
            search_input: &self.search_input,
            searching: self.searching,
            rules_path: &self.rules_path,
            rules_error: self.rules_error.as_deref(),
            status: &self.status,
        }
    }
}

fn move_table_sel(state: &mut TableState, len: usize, delta: i32) {
    if len == 0 {
        return;
    }
    let cur = state.selected().unwrap_or(0) as i32;
    let next = (cur + delta).rem_euclid(len as i32) as usize;
    state.select(Some(next));
}

fn app_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> anyhow::Result<()> {
    while !app.should_quit {
        redraw(terminal, app)?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.on_key(key, terminal)?;
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
