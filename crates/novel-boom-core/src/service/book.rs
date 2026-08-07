//! Book detail + TOC use-cases.

use crate::config::Config;
use crate::error::{Error, Result};
use crate::model::{apply_chapter_range, Book, Chapter, ChapterRange};
use crate::net::HttpClient;
use crate::parse::{fetch_book, fetch_toc};
use crate::rule::RuleCatalog;

/// Loaded book metadata with full table of contents.
#[derive(Debug, Clone)]
pub struct BookCatalog {
    pub book: Book,
    pub chapters: Vec<Chapter>,
}

/// Fetch detail page + full TOC for a book URL on the given source.
pub async fn fetch_book_catalog(
    client: &HttpClient,
    catalog: &RuleCatalog,
    config: &Config,
    source_id: u32,
    detail_url: &str,
) -> Result<BookCatalog> {
    let rule = catalog
        .get(source_id)
        .ok_or(Error::SourceNotFound(source_id))?;

    if rule.need_proxy && !config.proxy.enabled {
        return Err(Error::Message(format!(
            "书源「{}」需要代理，请在 config.toml 启用 [proxy]",
            rule.name
        )));
    }

    let detail_url = detail_url.trim();
    if detail_url.is_empty() {
        return Err(Error::Message("书籍详情地址为空".into()));
    }

    let book = fetch_book(client, rule, detail_url)
        .await
        .map_err(|err| annotate(&rule.name, "详情", err))?;
    let chapters = fetch_toc(client, rule, detail_url)
        .await
        .map_err(|err| annotate(&rule.name, "目录", err))?;

    Ok(BookCatalog { book, chapters })
}

/// Slice a loaded TOC according to user range selection.
pub fn select_chapters(chapters: &[Chapter], range: ChapterRange) -> Result<Vec<Chapter>> {
    apply_chapter_range(chapters, range)
}

fn annotate(source_name: &str, stage: &str, err: Error) -> Error {
    match err {
        Error::Http(msg) => Error::Http(format!("书源「{source_name}」{stage}失败\n{msg}")),
        Error::Message(msg) => Error::Message(format!("书源「{source_name}」{stage}失败：{msg}")),
        other => other,
    }
}
