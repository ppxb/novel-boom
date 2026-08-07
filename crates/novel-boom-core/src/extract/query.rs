//! Selector query helpers shared by parsers.

/// What to pull from a matched element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentKind {
    Text,
    Html,
    AttrHref,
    AttrSrc,
    AttrContent,
    AttrValue,
}

/// True when the rule field embeds `@js:` transforms.
pub fn requires_js(query: &str) -> bool {
    query.contains("@js:")
}

/// True when the primary selector looks like XPath.
pub fn requires_xpath(query: &str) -> bool {
    let selector = strip_extensions(query);
    let trimmed = selector.trim();
    trimmed.starts_with('/') || trimmed.starts_with('(')
}

/// Strip `@js:` / `@java:` tails and attribute markers for CSS selection.
///
/// Examples:
/// - `a@href` → `a`
/// - `#list a@js:code` → `#list a`
/// - `td.even > a` → `td.even > a`
pub fn strip_extensions(query: &str) -> &str {
    let without_lang = match query.find("@js:").or_else(|| query.find("@java:")) {
        Some(idx) => &query[..idx],
        None => query,
    };

    // Attribute markers: @href / @src (not language tags already stripped).
    if let Some(idx) = without_lang.rfind('@') {
        let after = &without_lang[idx + 1..];
        if after == "href" || after == "src" || after.starts_with("href") || after.starts_with("src")
        {
            return without_lang[..idx].trim_end();
        }
    }

    without_lang.trim_end()
}

/// Infer extraction kind from the original rule query string.
pub fn content_kind(query: &str) -> ContentKind {
    if query.contains("@href") {
        ContentKind::AttrHref
    } else if query.contains("@src") {
        ContentKind::AttrSrc
    } else if query.contains("@value") {
        ContentKind::AttrValue
    } else if query.trim_start().starts_with("meta[") {
        ContentKind::AttrContent
    } else {
        ContentKind::Text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_href_and_js() {
        assert_eq!(strip_extensions("td.even > a@href"), "td.even > a");
        assert_eq!(
            strip_extensions("#x@js:r=r.replace(/a/,'')"),
            "#x"
        );
        assert!(requires_js("a@js:return r"));
        assert!(requires_xpath("//div/a"));
        assert!(!requires_xpath("div > a"));
        assert_eq!(content_kind("a@href"), ContentKind::AttrHref);
        assert_eq!(content_kind("span.author"), ContentKind::Text);
    }
}
