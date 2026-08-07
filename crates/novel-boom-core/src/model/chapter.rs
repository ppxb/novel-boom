//! Single chapter entity.

/// One chapter in a table of contents or download queue.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Chapter {
    /// 1-based order in the selected download range.
    pub order: u32,
    pub title: String,
    pub url: String,
    /// Rendered body (HTML or plain text depending on pipeline stage).
    pub content: String,
}
