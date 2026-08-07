//! Search page fetch + HTML parsing.

use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use scraper::Html;
use url::Url;

use crate::error::{Error, Result};
use crate::extract::{
    extract, extract_content, requires_js, requires_xpath, select_all_in_document, strip_extensions,
    ContentKind,
};
use crate::model::SearchHit;
use crate::net::HttpClient;
use crate::rule::{Rule, SearchRule};

use super::form_data::parse_form_template;

/// Search a single book source.
pub async fn search_source(
    client: &HttpClient,
    rule: &Rule,
    keyword: &str,
    search_limit: u32,
) -> Result<Vec<SearchHit>> {
    if rule.disabled {
        return Err(Error::SourceDisabled(rule.name.clone()));
    }
    let search = rule
        .search
        .as_ref()
        .ok_or_else(|| Error::SearchNotSupported(rule.name.clone()))?;
    if search.disabled || search.url.trim().is_empty() {
        return Err(Error::SearchNotSupported(rule.name.clone()));
    }

    ensure_search_supported(search)?;

    let keyword = keyword.trim();
    if keyword.is_empty() {
        return Err(Error::Message("search keyword must not be empty".into()));
    }

    let base = resolve_base_uri(rule, search)?;
    let timeout = u64::from(search.timeout.unwrap_or(15));
    let cookies = empty_to_none(&search.cookies);

    let html = fetch_search_html(client, search, keyword, timeout, cookies).await?;
    let mut hits = parse_search_html(&html, rule, search, &base, search_limit)?;

    // Optional pagination (first-level page links only; no JS).
    if !search.next_page.trim().is_empty() && !requires_js(&search.next_page) {
        if let Ok(doc) = Ok::<_, Error>(Html::parse_document(&html)) {
            if let Ok(pages) = select_all_in_document(&doc, &search.next_page) {
                let mut urls = Vec::new();
                for el in pages {
                    let href = extract_content(el, ContentKind::AttrHref, &base);
                    if !href.is_empty() && !urls.contains(&href) {
                        urls.push(href);
                    }
                }

                let limit = effective_limit(search_limit);
                for page_url in urls {
                    if hits.len() >= limit {
                        break;
                    }
                    match client.get_text(&page_url, timeout, cookies).await {
                        Ok(page_html) => {
                            let more =
                                parse_search_html(&page_html, rule, search, &base, search_limit)?;
                            for hit in more {
                                if hits.len() >= limit {
                                    break;
                                }
                                if !hits.iter().any(|h| h.url == hit.url) {
                                    hits.push(hit);
                                }
                            }
                        }
                        Err(err) => {
                            tracing::warn!(url = %page_url, error = %err, "search page failed");
                        }
                    }
                }
            }
        }
    }

    let limit = effective_limit(search_limit);
    if hits.len() > limit {
        hits.truncate(limit);
    }
    Ok(hits)
}

/// Parse search result HTML without network I/O (for tests / fixtures).
pub fn parse_search_html(
    html: &str,
    rule: &Rule,
    search: &SearchRule,
    base: &Url,
    search_limit: u32,
) -> Result<Vec<SearchHit>> {
    ensure_search_supported(search)?;

    let document = Html::parse_document(html);
    let result_query = strip_extensions(&search.result);
    if result_query.is_empty() {
        return Err(Error::Message(format!(
            "source `{}` search.result is empty",
            rule.name
        )));
    }

    let rows = select_all_in_document(&document, result_query)?;
    let limit = effective_limit(search_limit);
    let mut hits = Vec::new();

    for row in rows {
        if hits.len() >= limit {
            break;
        }

        let book_name = extract(row, &search.book_name, base);
        if book_name.is_empty() {
            continue;
        }

        // Mirror so-novel: book URL always comes from bookName node @href.
        let url = {
            let css = strip_extensions(&search.book_name);
            if css.is_empty() {
                String::new()
            } else {
                match scraper::Selector::parse(css) {
                    Ok(sel) => row
                        .select(&sel)
                        .next()
                        .map(|el| extract_content(el, ContentKind::AttrHref, base))
                        .unwrap_or_default(),
                    Err(_) => String::new(),
                }
            }
        };

        if url.is_empty() {
            tracing::debug!(book = %book_name, "skip hit without detail url");
            continue;
        }

        hits.push(SearchHit {
            source_id: rule.id,
            source_name: rule.name.clone(),
            book_name,
            author: extract(row, &search.author, base),
            category: extract(row, &search.category, base),
            latest_chapter: extract(row, &search.latest_chapter, base),
            last_update_time: extract(row, &search.last_update_time, base),
            status: extract(row, &search.status, base),
            word_count: extract(row, &search.word_count, base),
            url,
        });
    }

    Ok(hits)
}

async fn fetch_search_html(
    client: &HttpClient,
    search: &SearchRule,
    keyword: &str,
    timeout: u64,
    cookies: Option<&str>,
) -> Result<String> {
    let method = search.method.trim().to_ascii_lowercase();
    let url = format_search_url(&search.url, keyword)?;

    if method == "post" {
        let form = parse_form_template(&search.data, keyword)?;
        client.post_form(&url, timeout, cookies, &form).await
    } else {
        // default GET
        client.get_text(&url, timeout, cookies).await
    }
}

fn format_search_url(template: &str, keyword: &str) -> Result<String> {
    if requires_js(template) {
        return Err(Error::UnsupportedFeature(
            "search.url embeds @js: (JavaScript not enabled yet)".into(),
        ));
    }
    if !template.contains("%s") {
        return Ok(template.to_string());
    }
    let encoded = utf8_percent_encode(keyword, NON_ALPHANUMERIC).to_string();
    Ok(template.replacen("%s", &encoded, 1))
}

fn resolve_base_uri(rule: &Rule, search: &SearchRule) -> Result<Url> {
    let raw = if search.base_uri.trim().is_empty() {
        rule.url.as_str()
    } else {
        search.base_uri.as_str()
    };
    Url::parse(raw).map_err(|err| Error::InvalidUrl(format!("{raw}: {err}")))
}

fn ensure_search_supported(search: &SearchRule) -> Result<()> {
    let fields = [
        ("url", search.url.as_str()),
        ("result", search.result.as_str()),
        ("bookName", search.book_name.as_str()),
        ("author", search.author.as_str()),
        ("category", search.category.as_str()),
        ("latestChapter", search.latest_chapter.as_str()),
        ("lastUpdateTime", search.last_update_time.as_str()),
        ("status", search.status.as_str()),
        ("wordCount", search.word_count.as_str()),
        ("nextPage", search.next_page.as_str()),
    ];

    for (name, value) in fields {
        if requires_js(value) {
            return Err(Error::UnsupportedFeature(format!(
                "search.{name} requires JavaScript (@js:), not enabled yet"
            )));
        }
        if !value.is_empty() && requires_xpath(value) {
            return Err(Error::UnsupportedFeature(format!(
                "search.{name} uses XPath, not enabled yet"
            )));
        }
    }
    Ok(())
}

fn effective_limit(search_limit: u32) -> usize {
    if search_limit == 0 {
        usize::MAX
    } else {
        search_limit as usize
    }
}

fn empty_to_none(value: &str) -> Option<&str> {
    let t = value.trim();
    if t.is_empty() { None } else { Some(t) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule::SearchRule;

    fn sample_rule() -> Rule {
        Rule {
            id: 1,
            url: "http://www.xbiqugu.la/".into(),
            name: "香书小说".into(),
            comment: String::new(),
            language: String::new(),
            need_proxy: false,
            disabled: false,
            search: Some(SearchRule {
                disabled: false,
                base_uri: String::new(),
                timeout: Some(15),
                url: "http://www.xbiqugu.la/modules/article/waps.php".into(),
                method: "post".into(),
                data: "{searchkey: %s}".into(),
                cookies: String::new(),
                result: "#checkform > table > tbody > tr".into(),
                book_name: "td.even > a".into(),
                author: "td:nth-of-type(3)".into(),
                category: String::new(),
                latest_chapter: "td.odd > a".into(),
                last_update_time: "td:nth-of-type(4)".into(),
                status: String::new(),
                word_count: String::new(),
                next_page: String::new(),
            }),
            book: None,
            toc: None,
            chapter: None,
            crawl: None,
        }
    }

    #[test]
    fn parse_fixture_rows() {
        let html = r#"
        <html><body>
          <form id="checkform"><table><tbody>
            <tr>
              <td class="even"><a href="/0_1/">诡秘之主</a></td>
              <td class="odd"><a href="/c">第1360章</a></td>
              <td>爱潜水的乌贼</td>
              <td>2020-05-01</td>
            </tr>
            <tr>
              <td class="even"><a href="/0_2/">宿命之环</a></td>
              <td class="odd"><a href="/c2">第100章</a></td>
              <td>爱潜水的乌贼</td>
              <td>2024-01-01</td>
            </tr>
          </tbody></table></form>
        </body></html>
        "#;

        let rule = sample_rule();
        let search = rule.search.as_ref().unwrap();
        let base = Url::parse(&rule.url).unwrap();
        let hits = parse_search_html(html, &rule, search, &base, 30).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].book_name, "诡秘之主");
        assert_eq!(hits[0].author, "爱潜水的乌贼");
        assert_eq!(hits[0].url, "http://www.xbiqugu.la/0_1/");
        assert_eq!(hits[0].latest_chapter, "第1360章");
        assert_eq!(hits[1].book_name, "宿命之环");
    }

    #[test]
    fn rejects_js_rules() {
        let mut rule = sample_rule();
        if let Some(search) = rule.search.as_mut() {
            search.url = "https://x.com/@js:return r".into();
        }
        let search = rule.search.as_ref().unwrap();
        let err = ensure_search_supported(search).unwrap_err();
        assert!(matches!(err, Error::UnsupportedFeature(_)));
    }
}
