//! 顶层界面状态。

/// 当前全屏页面。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// 主菜单。
    Home,
    /// 只读配置。
    Config,
    /// 书源一览。
    Sources,
    /// 尚未实现的功能占位。
    Placeholder(&'static str),
}
