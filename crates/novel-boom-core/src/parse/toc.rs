//! Table-of-contents parser.

use regex::Regex;
use scraper::{Html, Selector};
use url::Url;

use crate::error::{Error, Result};
use crate::extract::{
    extract_content, requires_js, requires_xpath, select_all_in_document, ContentKind,
};
use crate::model::Chapter;
use crate::net::HttpClient;
use crate::rule::{effective_book_rule, effective_toc_rule, Rule, TocRule};

const MAX_TOC_PAGES: usize = 80;

/// Fetch full TOC for a book detail URL.
pub async fn fetch_toc(client: &HttpClient, rule: &Rule, detail_url: &str) -> Result<Vec<Chapter>> {
    let mut toc_rule = effective_toc_rule(rule)
        .ok_or_else(|| Error::Message(format!("书源「{}」缺少 toc 规则", rule.name)))?;
    ensure_toc_supported(&toc_rule)?;

    let book_rule = effective_book_rule(rule);
    let book_id = extract_book_id(&book_rule.url, detail_url);

    if let Some(id) = book_id.as_deref() {
        if toc_rule.base_uri.contains("%s") {
            toc_rule.base_uri = toc_rule.base_uri.replacen("%s", id, 1);
        }
    }

    let mut first_url = detail_url.to_string();
    if !toc_rule.url.trim().is_empty() {
        let id = book_id.as_deref().ok_or_else(|| {
            Error::Message(format!(
                "书源「{}」配置了 toc.url，但无法从详情地址提取书籍 ID（book.url 正则）",
                rule.name
            ))
        })?;
        if requires_js(&toc_rule.url) {
            return Err(Error::UnsupportedFeature(
                "toc.url embeds @js: (not enabled yet)".into(),
            ));
        }
        first_url = toc_rule.url.replacen("%s", id, 1);
    }

    let timeout = u64::from(toc_rule.timeout.unwrap_or(60));
    let first_html = client.get_text(&first_url, timeout, None).await?;
    let first_base = resolve_toc_base(&toc_rule, rule, &first_url)?;
    let first_doc = Html::parse_document(&first_html);

    let mut page_urls = vec![first_url.clone()];
    if !toc_rule.next_page.trim().is_empty() {
        collect_pagination_urls(
            client,
            &first_doc,
            &first_html,
            &toc_rule,
            &first_base,
            timeout,
            &mut page_urls,
        )
        .await?;
    }

    let mut chapters = Vec::new();
    for (idx, url) in page_urls.iter().enumerate() {
        let html = if idx == 0 {
            first_html.clone()
        } else {
            client.get_text(url, timeout, None).await?
        };
        let page_base = resolve_toc_base(&toc_rule, rule, url)?;
        chapters.extend(parse_toc_html(&html, &toc_rule, &page_base)?);
    }

    if toc_rule.is_desc {
        chapters.reverse();
    }

    let mut cleaned = Vec::new();
    for ch in chapters {
        if ch.title.trim().is_empty() || ch.url.trim().is_empty() {
            continue;
        }
        if cleaned.iter().any(|c: &Chapter| c.url == ch.url) {
            continue;
        }
        cleaned.push(ch);
    }

    for (idx, ch) in cleaned.iter_mut().enumerate() {
        ch.order = (idx + 1) as u32;
    }

    if cleaned.is_empty() {
        return Err(Error::Message(format!(
            "书源「{}」目录解析结果为空（可能选择器失效或反爬）",
            rule.name
        )));
    }

    Ok(cleaned)
}

/// Parse a single TOC page HTML.
pub fn parse_toc_html(html: &str, toc_rule: &TocRule, base: &Url) -> Result<Vec<Chapter>> {
    ensure_toc_supported(toc_rule)?;
    let document = Html::parse_document(html);
    let elements = extract_toc_elements(&document, toc_rule)?;

    let mut chapters = Vec::with_capacity(elements.len());
    for el in elements {
        let title = normalize_ws(&el.text().collect::<String>());
        let url = extract_content(el, ContentKind::AttrHref, base);
        if title.is_empty() || url.is_empty() {
            continue;
        }
        chapters.push(Chapter {
            order: 0,
            title,
            url,
            content: String::new(),
        });
    }
    Ok(chapters)
}

fn extract_toc_elements<'a>(
    document: &'a Html,
    toc_rule: &TocRule,
) -> Result<Vec<scraper::ElementRef<'a>>> {
    if toc_rule.item.trim().is_empty() {
        return Err(Error::Message("toc.item 不能为空".into()));
    }

    // so-novel: when list is set, take that node's HTML then select items inside.
    if !toc_rule.list.trim().is_empty() {
        if requires_xpath(&toc_rule.list) || requires_js(&toc_rule.list) {
            return Err(Error::UnsupportedFeature(
                "toc.list requires unsupported selector feature".into(),
            ));
        }
        let css = crate::extract::strip_extensions(&toc_rule.list);
        let sel = Selector::parse(css).map_err(|err| Error::InvalidSelector {
            selector: css.to_string(),
            message: err.to_string(),
        })?;
        let list_html = document
            .select(&sel)
            .next()
            .map(|el| el.html())
            .unwrap_or_default();
        if list_html.trim().is_empty() {
            return Ok(Vec::new());
        }
        // Leak-free approach: parse fragment into owned Html stored... but we need
        // ElementRefs tied to document lifetime. Re-select items from a nested
        // temporary by re-querying item against a synthetic wrapper is awkward.
        // Practical approach: select item only under list via compound selector.
        let compound = format!("{css} {}", crate::extract::strip_extensions(&toc_rule.item));
        return select_all_in_document(document, &compound);
    }

    select_all_in_document(document, &toc_rule.item)
}

async fn collect_pagination_urls(
    client: &HttpClient,
    first_doc: &Html,
    first_html: &str,
    toc_rule: &TocRule,
    base: &Url,
    timeout: u64,
    urls: &mut Vec<String>,
) -> Result<()> {
    let Ok(elements) = select_all_in_document(first_doc, &toc_rule.next_page) else {
        return Ok(());
    };
    if elements.is_empty() {
        return Ok(());
    }

    let has_value = elements.iter().any(|el| el.value().attr("value").is_some());
    if has_value {
        for el in elements {
            let href = extract_content(el, ContentKind::AttrHref, base);
            let value = extract_content(el, ContentKind::AttrValue, base);
            let candidate = if !href.is_empty() { href } else { value };
            push_unique(urls, candidate);
        }
        return Ok(());
    }

    let mut document_html = first_html.to_string();
    for _ in 0..MAX_TOC_PAGES {
        let doc = Html::parse_document(&document_html);
        let els = select_all_in_document(&doc, &toc_rule.next_page).unwrap_or_default();
        let Some(el) = els.first() else {
            break;
        };
        let mut next = extract_content(*el, ContentKind::AttrHref, base);
        if next.is_empty() {
            next = extract_content(*el, ContentKind::AttrValue, base);
        }
        if next.is_empty() || !is_http_url(&next) || urls.contains(&next) {
            break;
        }
        urls.push(next.clone());
        document_html = client.get_text(&next, timeout, None).await?;
    }

    Ok(())
}

fn extract_book_id(book_url_pattern: &str, detail_url: &str) -> Option<String> {
    let pattern = book_url_pattern
        .split("@js:")
        .next()
        .unwrap_or(book_url_pattern)
        .trim();
    if pattern.is_empty() {
        return None;
    }
    let re = Regex::new(pattern).ok()?;
    let caps = re.captures(detail_url)?;
    caps.get(1).map(|m| m.as_str().to_string())
}

fn ensure_toc_supported(toc: &TocRule) -> Result<()> {
    for (name, value) in [
        ("url", toc.url.as_str()),
        ("list", toc.list.as_str()),
        ("item", toc.item.as_str()),
        ("nextPage", toc.next_page.as_str()),
    ] {
        if requires_js(value) {
            return Err(Error::UnsupportedFeature(format!(
                "toc.{name} requires @js: (not enabled yet)"
            )));
        }
        if !value.is_empty() && requires_xpath(value) {
            return Err(Error::UnsupportedFeature(format!(
                "toc.{name} uses XPath (not enabled yet)"
            )));
        }
    }
    Ok(())
}

fn resolve_toc_base(toc: &TocRule, rule: &Rule, page_url: &str) -> Result<Url> {
    let raw = if !toc.base_uri.trim().is_empty() {
        toc.base_uri.as_str()
    } else if !rule.url.trim().is_empty() {
        rule.url.as_str()
    } else {
        page_url
    };
    Url::parse(raw)
        .or_else(|_| Url::parse(page_url))
        .map_err(|err| Error::InvalidUrl(format!("{raw} / {page_url}: {err}")))
}

fn push_unique(urls: &mut Vec<String>, candidate: String) {
    let c = candidate.trim();
    if c.is_empty() || !is_http_url(c) {
        return;
    }
    if let Some(pos) = urls.iter().position(|u| u == c) {
        urls.remove(pos);
    }
    urls.push(c.to_string());
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn normalize_ws(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_toc_list() {
        let html = r#"
        <html><body>
          <div id="list"><dl>
            <dd><a href="/1.html">第一章 开始</a></dd>
            <dd><a href="/2.html">第二章 继续</a></dd>
          </dl></div>
        </body></html>
        "#;
        let toc = TocRule {
            base_uri: "http://www.example.com/".into(),
            timeout: Some(15),
            url: String::new(),
            list: String::new(),
            item: "#list dd > a".into(),
            is_desc: false,
            next_page: String::new(),
        };
        let base = Url::parse("http://www.example.com/").unwrap();
        let chapters = parse_toc_html(html, &toc, &base).unwrap();
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].title, "第一章 开始");
        assert_eq!(chapters[0].url, "http://www.example.com/1.html");
    }

    #[test]
    fn extracts_id_from_pattern() {
        let id = extract_book_id(
            "https://www.wxsy.net/novel/(.*?)/",
            "https://www.wxsy.net/novel/12345/",
        );
        assert_eq!(id.as_deref(), Some("12345"));
    }
}
