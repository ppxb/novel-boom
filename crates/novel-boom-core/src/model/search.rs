//! Search result row.

/// One hit returned by a source search.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchHit {
    pub source_id: u32,
    pub source_name: String,
    pub book_name: String,
    pub author: String,
    pub category: String,
    pub latest_chapter: String,
    pub last_update_time: String,
    pub status: String,
    pub word_count: String,
    /// Book detail URL used for subsequent TOC fetch.
    pub url: String,
}
