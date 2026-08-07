//! 顶层界面状态。

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
    /// 尚未实现的功能占位。
    Placeholder(&'static str),
}
