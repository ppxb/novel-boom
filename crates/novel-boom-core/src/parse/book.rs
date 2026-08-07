//! Book detail page parser.

use scraper::Html;
use url::Url;

use crate::error::{Error, Result};
use crate::extract::{extract_in_document, requires_js, requires_xpath};
use crate::model::Book;
use crate::net::HttpClient;
use crate::rule::{effective_book_rule, BookRule, Rule};

/// Fetch and parse a book detail page.
pub async fn fetch_book(client: &HttpClient, rule: &Rule, detail_url: &str) -> Result<Book> {
    let book_rule = effective_book_rule(rule);
    ensure_book_supported(&book_rule)?;

    let timeout = u64::from(book_rule.timeout.unwrap_or(15));
    let html = client.get_text(detail_url, timeout, None).await?;
    let base = resolve_base(&book_rule.base_uri, &rule.url, detail_url)?;
    parse_book_html(&html, rule, &book_rule, detail_url, &base)
}

/// Parse detail HTML without network I/O.
pub fn parse_book_html(
    html: &str,
    rule: &Rule,
    book_rule: &BookRule,
    detail_url: &str,
    base: &Url,
) -> Result<Book> {
    ensure_book_supported(book_rule)?;
    let document = Html::parse_document(html);

    let name = extract_in_document(&document, &book_rule.book_name, base);
    let mut author = extract_in_document(&document, &book_rule.author, base);
    author = author.replace("作者：", "").replace("作者:", "");
    author = author.trim().to_string();

    if name.trim().is_empty() || author.is_empty() {
        return Err(Error::Message(format!(
            "详情页书名或作者为空（书源「{}」）",
            rule.name
        )));
    }

    let mut last_update = extract_in_document(&document, &book_rule.last_update_time, base);
    for prefix in ["更新时间：", "更新时间:", "最后更新：", "最后更新:"] {
        if let Some(rest) = last_update.strip_prefix(prefix) {
            last_update = rest.trim().to_string();
            break;
        }
    }

    let cover_raw = extract_in_document(&document, &book_rule.cover_url, base);
    let latest_url_raw =
        extract_in_document(&document, &book_rule.latest_chapter_url, base);

    Ok(Book {
        name: name.trim().to_string(),
        author,
        intro: collapse_ws(&extract_in_document(&document, &book_rule.intro, base)),
        category: extract_in_document(&document, &book_rule.category, base),
        cover_url: absolutize_url(&cover_raw, base),
        latest_chapter: extract_in_document(&document, &book_rule.latest_chapter, base),
        latest_chapter_url: absolutize_url(&latest_url_raw, base),
        last_update_time: last_update,
        status: extract_in_document(&document, &book_rule.status, base),
        url: detail_url.to_string(),
        source_id: rule.id,
        source_name: rule.name.clone(),
    })
}

fn absolutize_url(raw: &str, base: &Url) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return String::new();
    }
    if raw.starts_with("http://") || raw.starts_with("https://") || raw.starts_with("data:") {
        return raw.to_string();
    }
    base.join(raw).map(Into::into).unwrap_or_else(|_| raw.to_string())
}

fn ensure_book_supported(book: &BookRule) -> Result<()> {
    for (name, value) in [
        ("bookName", book.book_name.as_str()),
        ("author", book.author.as_str()),
        ("intro", book.intro.as_str()),
        ("coverUrl", book.cover_url.as_str()),
        ("category", book.category.as_str()),
        ("latestChapter", book.latest_chapter.as_str()),
        ("latestChapterUrl", book.latest_chapter_url.as_str()),
        ("lastUpdateTime", book.last_update_time.as_str()),
        ("status", book.status.as_str()),
    ] {
        if requires_js(value) {
            return Err(Error::UnsupportedFeature(format!(
                "book.{name} requires @js: (not enabled yet)"
            )));
        }
        if !value.is_empty() && requires_xpath(value) {
            return Err(Error::UnsupportedFeature(format!(
                "book.{name} uses XPath (not enabled yet)"
            )));
        }
    }
    Ok(())
}

fn resolve_base(base_uri: &str, rule_url: &str, detail_url: &str) -> Result<Url> {
    let raw = if !base_uri.trim().is_empty() {
        base_uri
    } else if !rule_url.trim().is_empty() {
        rule_url
    } else {
        detail_url
    };
    Url::parse(raw).map_err(|err| Error::InvalidUrl(format!("{raw}: {err}")))
}

fn collapse_ws(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule::Rule;

    #[test]
    fn parses_meta_defaults() {
        let html = r#"
        <html><head>
          <meta property="og:novel:book_name" content="诡秘之主"/>
          <meta property="og:novel:author" content="爱潜水的乌贼"/>
          <meta name="description" content="简介文字"/>
          <meta property="og:image" content="/cover.jpg"/>
        </head><body></body></html>
        "#;
        let rule = Rule {
            id: 1,
            url: "http://example.com/".into(),
            name: "测试源".into(),
            comment: String::new(),
            language: String::new(),
            need_proxy: false,
            disabled: false,
            search: None,
            book: None,
            toc: None,
            chapter: None,
            crawl: None,
        };
        let book_rule = effective_book_rule(&rule);
        let base = Url::parse("http://example.com/").unwrap();
        let book = parse_book_html(html, &rule, &book_rule, "http://example.com/book/1/", &base)
            .unwrap();
        assert_eq!(book.name, "诡秘之主");
        assert_eq!(book.author, "爱潜水的乌贼");
        assert_eq!(book.cover_url, "http://example.com/cover.jpg");
    }
}
