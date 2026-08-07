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
use novel_boom_core::model::{Chapter, ChapterRange, SearchHit};
use novel_boom_core::net::HttpClient;
use novel_boom_core::rule::{RuleCatalog, SourceInfo};
use novel_boom_core::service::{self, BookCatalog};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::{ListState, TableState};
use tokio::runtime::Runtime;

use crate::screen::{RangeInputMode, Screen};
use crate::ui::{self, UiModel};

/// TUI 启动参数。
pub struct TuiOptions {
    pub config: Config,
    pub catalog: Option<RuleCatalog>,
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
    toc_state: TableState,
    sources: Vec<SourceInfo>,
    searchable_sources: Vec<SourceInfo>,
    search_input: String,
    range_input: String,
    search_hits: Vec<SearchHit>,
    book_catalog: Option<BookCatalog>,
    selected_chapters: Vec<Chapter>,
    busy: bool,
    busy_message: String,
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
            toc_state: TableState::default(),
            sources,
            searchable_sources,
            search_input: String::new(),
            range_input: String::new(),
            search_hits: Vec::new(),
            book_catalog: None,
            selected_chapters: Vec::new(),
            busy: false,
            busy_message: String::new(),
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
        if key.kind != KeyEventKind::Press || self.busy {
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
            Screen::SingleSearchResults { .. } => self.on_search_results_key(key, terminal)?,
            Screen::BookToc { .. } => self.on_book_toc_key(key),
            Screen::BookRangeInput { .. } => self.on_range_input_key(key),
            Screen::DownloadPlan { .. } => self.on_download_plan_key(key),
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

    fn on_search_results_key(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> anyhow::Result<()> {
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
            KeyCode::Enter => self.open_selected_book(terminal)?,
            _ => {}
        }
        Ok(())
    }

    fn on_book_toc_key(&mut self, key: KeyEvent) {
        let toc_len = self
            .book_catalog
            .as_ref()
            .map(|c| c.chapters.len())
            .unwrap_or(0);

        match key.code {
            KeyCode::Esc => self.back_to_search_results(),
            KeyCode::Up | KeyCode::Char('k') => move_table_sel(&mut self.toc_state, toc_len, -1),
            KeyCode::Down | KeyCode::Char('j') => move_table_sel(&mut self.toc_state, toc_len, 1),
            KeyCode::Char('1') => {
                let _ = self.apply_range(ChapterRange::All, "全本".into());
            }
            KeyCode::Char('2') => self.open_range_input(RangeInputMode::Span),
            KeyCode::Char('3') => self.open_range_input(RangeInputMode::Latest),
            _ => {}
        }
    }

    fn on_range_input_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                if let Screen::BookRangeInput {
                    source_id,
                    source_name,
                    keyword,
                    ..
                } = &self.screen
                {
                    self.screen = Screen::BookToc {
                        source_id: *source_id,
                        source_name: source_name.clone(),
                        keyword: keyword.clone(),
                    };
                    self.status = "已返回目录".into();
                }
            }
            KeyCode::Enter => self.confirm_range_input(),
            KeyCode::Backspace => {
                self.range_input.pop();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.range_input.clear();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.range_input.push(c);
            }
            _ => {}
        }
    }

    fn on_download_plan_key(&mut self, key: KeyEvent) {
        if matches!(
            key.code,
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')
        ) {
            if let Screen::DownloadPlan {
                source_id,
                source_name,
                keyword,
                ..
            } = &self.screen
            {
                self.screen = Screen::BookToc {
                    source_id: *source_id,
                    source_name: source_name.clone(),
                    keyword: keyword.clone(),
                };
                self.status = "已返回目录，可重新选择范围".into();
            }
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
        self.book_catalog = None;
        self.selected_chapters.clear();
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
        let Some(catalog) = self.catalog.clone() else {
            self.status = "书源未加载".into();
            return Ok(());
        };

        self.set_busy(true, format!("正在搜索「{keyword}」…"));
        redraw(terminal, self)?;

        let result = self.runtime.block_on(service::single_search(
            &self.http,
            &catalog,
            &self.config,
            source_id,
            &keyword,
        ));
        self.set_busy(false, String::new());

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
                    "搜索完成 — [{source_id}] {source_name} · 「{keyword}」· {count} 条 · Enter 打开"
                );
            }
            Err(err) => self.status = format!("搜索失败：{err}"),
        }
        Ok(())
    }

    fn open_selected_book(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> anyhow::Result<()> {
        let Screen::SingleSearchResults {
            source_id,
            source_name,
            keyword,
        } = &self.screen
        else {
            return Ok(());
        };
        let source_id = *source_id;
        let source_name = source_name.clone();
        let keyword = keyword.clone();

        let Some(hit) = self
            .search_result_state
            .selected()
            .and_then(|i| self.search_hits.get(i))
            .cloned()
        else {
            self.status = "请先选择一条搜索结果".into();
            return Ok(());
        };
        let Some(catalog) = self.catalog.clone() else {
            self.status = "书源未加载".into();
            return Ok(());
        };

        self.set_busy(
            true,
            format!("正在解析《{}》详情与目录…", hit.book_name),
        );
        redraw(terminal, self)?;

        let result = self.runtime.block_on(service::fetch_book_catalog(
            &self.http,
            &catalog,
            &self.config,
            source_id,
            &hit.url,
        ));
        self.set_busy(false, String::new());

        match result {
            Ok(book_catalog) => {
                let count = book_catalog.chapters.len();
                self.toc_state
                    .select(if count > 0 { Some(0) } else { None });
                self.selected_chapters.clear();
                self.book_catalog = Some(book_catalog);
                self.screen = Screen::BookToc {
                    source_id,
                    source_name: source_name.clone(),
                    keyword,
                };
                self.status = format!(
                    "目录已加载 — 《{}》共 {count} 章 · 1 全本 / 2 范围 / 3 最新",
                    self.book_catalog
                        .as_ref()
                        .map(|c| c.book.name.as_str())
                        .unwrap_or("?")
                );
            }
            Err(err) => self.status = format!("打开书籍失败：{err}"),
        }
        Ok(())
    }

    fn open_range_input(&mut self, mode: RangeInputMode) {
        let Screen::BookToc {
            source_id,
            source_name,
            keyword,
        } = &self.screen
        else {
            return;
        };
        self.range_input.clear();
        self.screen = Screen::BookRangeInput {
            source_id: *source_id,
            source_name: source_name.clone(),
            keyword: keyword.clone(),
            mode,
        };
        self.status = match mode {
            RangeInputMode::Span => "请输入起始章与结束章，例如：1 50".into(),
            RangeInputMode::Latest => "请输入最新章节数量，例如：20".into(),
        };
    }

    fn confirm_range_input(&mut self) {
        let Screen::BookRangeInput { mode, .. } = &self.screen else {
            return;
        };
        let mode = *mode;
        let raw = self.range_input.trim().to_string();
        if raw.is_empty() {
            self.status = "输入不能为空".into();
            return;
        }

        let range = match mode {
            RangeInputMode::Span => {
                let parts: Vec<_> = raw.split_whitespace().collect();
                if parts.len() != 2 {
                    self.status = "格式错误：请输入两个数字，例如 1 50".into();
                    return;
                }
                let Ok(start) = parts[0].parse::<u32>() else {
                    self.status = "起始章不是有效数字".into();
                    return;
                };
                let Ok(end) = parts[1].parse::<u32>() else {
                    self.status = "结束章不是有效数字".into();
                    return;
                };
                ChapterRange::Span { start, end }
            }
            RangeInputMode::Latest => {
                let Ok(count) = raw.parse::<u32>() else {
                    self.status = "数量不是有效数字".into();
                    return;
                };
                ChapterRange::Latest { count }
            }
        };

        let label = match range {
            ChapterRange::All => "全本".into(),
            ChapterRange::Span { start, end } => format!("第 {start}-{end} 章"),
            ChapterRange::Latest { count } => format!("最新 {count} 章"),
        };
        let _ = self.apply_range(range, label);
    }

    fn apply_range(&mut self, range: ChapterRange, label: String) -> bool {
        let Some(catalog) = self.book_catalog.as_ref() else {
            self.status = "目录未加载".into();
            return false;
        };
        match service::select_chapters(&catalog.chapters, range) {
            Ok(selected) => {
                let count = selected.len();
                self.selected_chapters = selected;
                if let Screen::BookToc {
                    source_id,
                    source_name,
                    keyword,
                }
                | Screen::BookRangeInput {
                    source_id,
                    source_name,
                    keyword,
                    ..
                } = &self.screen
                {
                    self.screen = Screen::DownloadPlan {
                        source_id: *source_id,
                        source_name: source_name.clone(),
                        keyword: keyword.clone(),
                        range_label: label.clone(),
                    };
                }
                self.status = format!("已选定 {count} 章（{label}）· 下载功能即将推出");
                true
            }
            Err(err) => {
                self.status = format!("范围无效：{err}");
                false
            }
        }
    }

    fn back_to_search_results(&mut self) {
        if let Screen::BookToc {
            source_id,
            source_name,
            keyword,
        } = &self.screen
        {
            self.screen = Screen::SingleSearchResults {
                source_id: *source_id,
                source_name: source_name.clone(),
                keyword: keyword.clone(),
            };
            self.status = "已返回搜索结果".into();
        }
    }

    fn back_home(&mut self, status: &str) {
        self.screen = Screen::Home;
        self.status = status.into();
    }

    fn set_busy(&mut self, busy: bool, message: String) {
        self.busy = busy;
        self.busy_message = message;
        if busy {
            self.status = self.busy_message.clone();
        }
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
            range_input: &self.range_input,
            book: self.book_catalog.as_ref().map(|c| &c.book),
            toc_chapters: self
                .book_catalog
                .as_ref()
                .map(|c| c.chapters.as_slice())
                .unwrap_or(&[]),
            selected_chapters: &self.selected_chapters,
            toc_state: &self.toc_state,
            busy: self.busy,
            busy_message: &self.busy_message,
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
