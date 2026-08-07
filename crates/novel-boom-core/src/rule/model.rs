//! Book-source rule definitions (so-novel JSON compatible).

use serde::Deserialize;

/// One site rule from a rules pack JSON array.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    /// Assigned 1-based id after load (JSON may omit this).
    #[serde(default)]
    pub id: u32,
    /// Site base URL used for matching and defaults.
    pub url: String,
    /// Human-readable source name.
    pub name: String,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub need_proxy: bool,
    #[serde(default)]
    pub disabled: bool,
    pub search: Option<SearchRule>,
    pub book: Option<BookRule>,
    pub toc: Option<TocRule>,
    pub chapter: Option<ChapterRule>,
    pub crawl: Option<CrawlRule>,
}

impl Rule {
    /// Whether this source can participate in keyword search.
    pub fn is_searchable(&self) -> bool {
        if self.disabled {
            return false;
        }
        match &self.search {
            Some(search) => !search.disabled && !search.url.trim().is_empty(),
            None => false,
        }
    }
}

/// Search-page extraction rule.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRule {
    /// When true, exclude from aggregated search.
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub base_uri: String,
    pub timeout: Option<u32>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub data: String,
    #[serde(default)]
    pub cookies: String,
    #[serde(default)]
    pub result: String,
    #[serde(default)]
    pub book_name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub latest_chapter: String,
    #[serde(default)]
    pub last_update_time: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub word_count: String,
    #[serde(default)]
    pub next_page: String,
}

/// Book detail-page extraction rule.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookRule {
    #[serde(default)]
    pub base_uri: String,
    pub timeout: Option<u32>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub book_name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub intro: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub cover_url: String,
    #[serde(default)]
    pub latest_chapter: String,
    #[serde(default)]
    pub latest_chapter_url: String,
    #[serde(default)]
    pub last_update_time: String,
    #[serde(default)]
    pub status: String,
}

/// Table-of-contents extraction rule.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TocRule {
    #[serde(default)]
    pub base_uri: String,
    pub timeout: Option<u32>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub list: String,
    #[serde(default)]
    pub item: String,
    #[serde(default)]
    pub is_desc: bool,
    #[serde(default)]
    pub next_page: String,
}

/// Chapter-page extraction rule.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterRule {
    #[serde(default)]
    pub base_uri: String,
    pub timeout: Option<u32>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub paragraph_tag_closed: bool,
    #[serde(default)]
    pub paragraph_tag: String,
    #[serde(default)]
    pub filter_txt: String,
    #[serde(default)]
    pub filter_tag: String,
    #[serde(default)]
    pub next_page: String,
    #[serde(default)]
    pub next_page_in_js: String,
    #[serde(default)]
    pub next_chapter_link: String,
}

/// Optional per-source crawl limits (overrides global config when set).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrawlRule {
    pub concurrency: Option<u32>,
    pub min_interval: Option<u64>,
    pub max_interval: Option<u64>,
    pub max_attempts: Option<u32>,
    pub retry_min_interval: Option<u64>,
    pub retry_max_interval: Option<u64>,
}
