//! CSS selection and field extraction via `scraper`.

use scraper::{ElementRef, Html, Selector};
use url::Url;

use crate::error::{Error, Result};

use super::query::{ContentKind, content_kind, requires_xpath, strip_extensions};

/// Parse a CSS selector, rejecting XPath-looking queries for now.
pub fn compile_css(query: &str) -> Result<Selector> {
    if requires_xpath(query) {
        return Err(Error::UnsupportedFeature(format!(
            "XPath selector not supported yet: {query}"
        )));
    }
    let css = strip_extensions(query);
    if css.is_empty() {
        return Err(Error::InvalidSelector {
            selector: query.to_string(),
            message: "empty selector".into(),
        });
    }
    Selector::parse(css).map_err(|err| Error::InvalidSelector {
        selector: css.to_string(),
        message: err.to_string(),
    })
}

/// All elements matching `query` under `root` (document or element scope).
pub fn select_all<'a>(root: ElementRef<'a>, query: &str) -> Result<Vec<ElementRef<'a>>> {
    let selector = compile_css(query)?;
    Ok(root.select(&selector).collect())
}

/// First match under a full HTML document.
pub fn select_all_in_document<'a>(document: &'a Html, query: &str) -> Result<Vec<ElementRef<'a>>> {
    let selector = compile_css(query)?;
    Ok(document.select(&selector).collect())
}

/// First match in document, if any.
pub fn select_first<'a>(document: &'a Html, query: &str) -> Result<Option<ElementRef<'a>>> {
    Ok(select_all_in_document(document, query)?.into_iter().next())
}

/// Extract a field from an element using rule query semantics.
pub fn extract(element: ElementRef<'_>, query: &str, base: &Url) -> String {
    if query.trim().is_empty() {
        return String::new();
    }

    let kind = content_kind(query);
    let css = strip_extensions(query);

    // When query points at a descendant, resolve relative to `element`.
    let target = if css.is_empty() {
        Some(element)
    } else {
        match Selector::parse(css) {
            Ok(sel) => element.select(&sel).next().or(Some(element)),
            Err(_) => Some(element),
        }
    };

    let Some(node) = target else {
        return String::new();
    };
    extract_content(node, kind, base)
}

/// Extract using an already-selected node (no nested CSS).
pub fn extract_content(element: ElementRef<'_>, kind: ContentKind, base: &Url) -> String {
    match kind {
        ContentKind::Text => normalize_ws(&element.text().collect::<String>()),
        ContentKind::Html => element.html(),
        ContentKind::AttrHref => abs_url(element, "href", base),
        ContentKind::AttrSrc => abs_url(element, "src", base),
        ContentKind::AttrContent => element
            .value()
            .attr("content")
            .unwrap_or_default()
            .trim()
            .to_string(),
    }
}

fn abs_url(element: ElementRef<'_>, attr: &str, base: &Url) -> String {
    let Some(raw) = element.value().attr(attr) else {
        return String::new();
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return String::new();
    }
    match base.join(raw) {
        Ok(url) => url.into(),
        Err(_) => raw.to_string(),
    }
}

fn normalize_ws(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::Selector;

    #[test]
    fn extracts_book_row() {
        // Tables rows are often reparented; wrap in a full table.
        let html = Html::parse_document(
            r#"
            <html><body>
              <table><tbody>
                <tr class="row">
                  <td class="even"><a href="/book/1/">诡秘之主</a></td>
                  <td>爱潜水的乌贼</td>
                  <td class="odd"><a href="/c/9">第1360章</a></td>
                </tr>
              </tbody></table>
            </body></html>
            "#,
        );
        let base = Url::parse("http://www.xbiqugu.la/").unwrap();
        let row_sel = Selector::parse("tr.row").unwrap();
        let row = html.select(&row_sel).next().expect("row present");

        assert_eq!(extract(row, "td.even > a", &base), "诡秘之主");
        assert_eq!(
            extract_content(
                row.select(&Selector::parse("td.even > a").unwrap())
                    .next()
                    .unwrap(),
                ContentKind::AttrHref,
                &base
            ),
            "http://www.xbiqugu.la/book/1/"
        );
        assert_eq!(extract(row, "td:nth-of-type(2)", &base), "爱潜水的乌贼");
    }
}
