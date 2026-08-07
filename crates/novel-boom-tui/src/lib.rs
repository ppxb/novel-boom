//! novel-boom 终端界面。
//!
//! 仅负责展示与交互，依赖 [`novel_boom_core`] 提供的配置、书源与用例。
//! 本 crate 不实现 HTML 解析、HTTP 爬取或导出逻辑。

mod app;
mod screen;
mod ui;

pub use app::{TuiOptions, run};
