//! Parse so-novel style form templates: `{key: value, key2: %s}`.

use crate::error::{Error, Result};

/// Build ordered form fields from a rule `data` template.
///
/// Each `%s` value is replaced by the next argument (usually the keyword).
pub fn parse_form_template(template: &str, keyword: &str) -> Result<Vec<(String, String)>> {
    let trimmed = template.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let body = trimmed
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .ok_or_else(|| {
            Error::Message(format!(
                "search data must look like {{key: value}}, got: {template}"
            ))
        })?;

    let mut fields = Vec::new();
    let mut args_used = 0usize;

    for part in split_top_level_commas(body) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (key, value) = part.split_once(':').ok_or_else(|| {
            Error::Message(format!("invalid form field `{part}` in `{template}`"))
        })?;
        let key = key.trim().trim_matches(['\'', '"']).to_string();
        let mut value = value.trim().trim_matches(['\'', '"']).to_string();
        if value == "%s" {
            value = keyword.to_string();
            args_used += 1;
        }
        fields.push((key, value));
    }

    if args_used == 0 && template.contains("%s") {
        // Rare: `%s` embedded inside a larger value — replace once.
        for (_, value) in &mut fields {
            if value.contains("%s") {
                *value = value.replacen("%s", keyword, 1);
                break;
            }
        }
    }

    Ok(fields)
}

fn split_top_level_commas(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    for (idx, ch) in input.char_indices() {
        match ch {
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&input[start..idx]);
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&input[start..]);
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_templates() {
        let fields = parse_form_template("{searchkey: %s}", "诡秘").unwrap();
        assert_eq!(fields, vec![("searchkey".into(), "诡秘".into())]);

        let fields =
            parse_form_template("{searchkey: %s, searchtype: all}", "测试").unwrap();
        assert_eq!(
            fields,
            vec![
                ("searchkey".into(), "测试".into()),
                ("searchtype".into(), "all".into()),
            ]
        );

        let fields = parse_form_template("{type: articlename, s: %s}", "abc").unwrap();
        assert_eq!(fields[0].1, "articlename");
        assert_eq!(fields[1], ("s".into(), "abc".into()));
    }
}
