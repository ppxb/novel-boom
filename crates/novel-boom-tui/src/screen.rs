//! 顶层界面状态。

/// 章节范围输入模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeInputMode {
    /// 起始章 结束章（空格分隔，从 1 起）。
    Span,
    /// 最新 N 章。
    Latest,
}

/// 当前全屏页面。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    /// 主菜单。
    Home,
    /// 只读配置。
    Config,
    /// 书源一览。
    Sources,
    /// 独立搜索：选择书源。
    SingleSearchPickSource,
    /// 独立搜索：输入关键词。
    SingleSearchInput {
        source_id: u32,
        source_name: String,
    },
    /// 独立搜索：展示结果。
    SingleSearchResults {
        source_id: u32,
        source_name: String,
        keyword: String,
    },
    /// 书籍详情 + 目录。
    BookToc {
        source_id: u32,
        source_name: String,
        keyword: String,
    },
    /// 输入章节范围。
    BookRangeInput {
        source_id: u32,
        source_name: String,
        keyword: String,
        mode: RangeInputMode,
    },
    /// 已选定章节（下载占位）。
    DownloadPlan {
        source_id: u32,
        source_name: String,
        keyword: String,
        range_label: String,
    },
    /// 尚未实现的功能占位。
    Placeholder(&'static str),
}
