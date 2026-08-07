//! Default selectors aligned with so-novel `SourceUtils.applyDefaultRule`.

use crate::rule::{BookRule, Rule, SearchRule, TocRule};

const META_BOOK_NAME: &str = r#"meta[property="og:novel:book_name"]"#;
const META_AUTHOR: &str = r#"meta[property="og:novel:author"]"#;
const META_INTRO: &str = r#"meta[name="description"]"#;
const META_CATEGORY: &str = r#"meta[property="og:novel:category"]"#;
const META_COVER_URL: &str = r#"meta[property="og:image"]"#;
const META_LATEST_CHAPTER: &str = r#"meta[property="og:novel:latest_chapter_name"]"#;
const META_LATEST_CHAPTER_URL: &str = r#"meta[property="og:novel:latest_chapter_url"]"#;
const META_LAST_UPDATE_TIME: &str = r#"meta[property="og:novel:update_time"]"#;
const META_STATUS: &str = r#"meta[property="og:novel:status"]"#;

/// Fill empty book selectors with Open Graph meta defaults.
pub fn effective_book_rule(rule: &Rule) -> BookRule {
    let mut book = rule.book.clone().unwrap_or(BookRule {
        base_uri: String::new(),
        timeout: None,
        url: String::new(),
        book_name: String::new(),
        author: String::new(),
        intro: String::new(),
        category: String::new(),
        cover_url: String::new(),
        latest_chapter: String::new(),
        latest_chapter_url: String::new(),
        last_update_time: String::new(),
        status: String::new(),
    });

    if book.base_uri.trim().is_empty() {
        book.base_uri = rule.url.clone();
    }
    if book.timeout.is_none() {
        book.timeout = Some(15);
    }
    fill_if_empty(&mut book.book_name, META_BOOK_NAME);
    fill_if_empty(&mut book.author, META_AUTHOR);
    fill_if_empty(&mut book.intro, META_INTRO);
    fill_if_empty(&mut book.category, META_CATEGORY);
    fill_if_empty(&mut book.cover_url, META_COVER_URL);
    fill_if_empty(&mut book.latest_chapter, META_LATEST_CHAPTER);
    fill_if_empty(&mut book.latest_chapter_url, META_LATEST_CHAPTER_URL);
    fill_if_empty(&mut book.last_update_time, META_LAST_UPDATE_TIME);
    fill_if_empty(&mut book.status, META_STATUS);
    book
}

/// Ensure toc has base_uri / timeout defaults.
pub fn effective_toc_rule(rule: &Rule) -> Option<TocRule> {
    let mut toc = rule.toc.clone()?;
    if toc.base_uri.trim().is_empty() {
        toc.base_uri = rule.url.clone();
    }
    if toc.timeout.is_none() {
        toc.timeout = Some(60);
    }
    Some(toc)
}

/// Ensure search has base_uri / timeout defaults.
#[allow(dead_code)]
pub fn effective_search_rule(rule: &Rule) -> Option<SearchRule> {
    let mut search = rule.search.clone()?;
    if search.base_uri.trim().is_empty() {
        search.base_uri = rule.url.clone();
    }
    if search.timeout.is_none() {
        search.timeout = Some(15);
    }
    Some(search)
}

fn fill_if_empty(field: &mut String, default: &str) {
    if field.trim().is_empty() {
        *field = default.to_string();
    }
}
