//! Book metadata resolved from a detail page.

/// A novel's identity and catalog-facing fields.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Book {
    pub name: String,
    pub author: String,
    pub intro: String,
    pub category: String,
    pub cover_url: String,
    pub latest_chapter: String,
    pub latest_chapter_url: String,
    pub last_update_time: String,
    pub status: String,
    /// Detail / catalog entry URL.
    pub url: String,
    pub source_id: u32,
    pub source_name: String,
}
