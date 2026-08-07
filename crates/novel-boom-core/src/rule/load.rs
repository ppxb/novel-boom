//! Load so-novel-compatible rules JSON packs.

use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::{Error, Result};

use super::model::Rule;

/// Default directory name for rule packs next to CWD / config.
pub const DEFAULT_RULES_DIR: &str = "rules";

/// Resolve the path of the active rules file.
///
/// Lookup order when `active_rules` is relative:
/// 1. `{config_dir}/rules/{active_rules}` when `config_path` is set
/// 2. `./rules/{active_rules}`
/// 3. `./{active_rules}`
///
/// Absolute `active_rules` is used as-is.
pub fn resolve_rules_path(config: &Config, config_path: Option<&Path>) -> PathBuf {
    let active = config.source.active_rules.trim();
    let active_path = Path::new(active);
    if active_path.is_absolute() {
        return active_path.to_path_buf();
    }

    let mut candidates = Vec::new();

    if let Some(cfg_path) = config_path {
        if let Some(parent) = cfg_path.parent() {
            candidates.push(parent.join(DEFAULT_RULES_DIR).join(active));
            candidates.push(parent.join(active));
        }
    }

    candidates.push(PathBuf::from(DEFAULT_RULES_DIR).join(active));
    candidates.push(PathBuf::from(active));

    for path in &candidates {
        if path.exists() {
            return path.clone();
        }
    }

    // Prefer the conventional location in error messages.
    candidates
        .into_iter()
        .next()
        .unwrap_or_else(|| PathBuf::from(DEFAULT_RULES_DIR).join(active))
}

/// Load rules from an explicit JSON file path.
///
/// Assigns 1-based sequential ids (same as so-novel).
pub fn load_rules_file(path: impl AsRef<Path>) -> Result<Vec<Rule>> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(Error::RulesNotFound(path.to_path_buf()));
    }

    let raw = fs::read_to_string(path)?;
    let mut rules: Vec<Rule> =
        serde_json::from_str(&raw).map_err(|err| Error::InvalidRules {
            path: path.to_path_buf(),
            source: Box::new(err),
        })?;

    for (index, rule) in rules.iter_mut().enumerate() {
        rule.id = u32::try_from(index + 1).unwrap_or(u32::MAX);
    }

    tracing::info!(
        path = %path.display(),
        count = rules.len(),
        "loaded book source rules"
    );

    Ok(rules)
}

/// Load the active rules pack referenced by `config`.
pub fn load_active_rules(config: &Config, config_path: Option<&Path>) -> Result<Vec<Rule>> {
    let path = resolve_rules_path(config, config_path);
    load_rules_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn load_assigns_sequential_ids() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        write!(
            file,
            r#"[
              {{
                "url": "https://a.example/",
                "name": "源A",
                "search": {{ "url": "https://a.example/s", "method": "get" }}
              }},
              {{
                "url": "https://b.example/",
                "name": "源B",
                "disabled": true
              }}
            ]"#
        )
        .unwrap();

        let rules = load_rules_file(file.path()).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].id, 1);
        assert_eq!(rules[0].name, "源A");
        assert!(rules[0].is_searchable());
        assert_eq!(rules[1].id, 2);
        assert!(rules[1].disabled);
        assert!(!rules[1].is_searchable());
    }

    #[test]
    fn missing_file_errors() {
        let err = load_rules_file("no-such-rules-pack.json").unwrap_err();
        assert!(matches!(err, Error::RulesNotFound(_)));
    }

    #[test]
    fn load_workspace_main_json_if_present() {
        // crates/novel-boom-core -> novel-boom/rules/main.json
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../rules/main.json");
        if !path.exists() {
            return;
        }
        let rules = load_rules_file(&path).unwrap();
        assert!(
            rules.len() >= 5,
            "expected several sources in main.json, got {}",
            rules.len()
        );
        assert_eq!(rules[0].id, 1);
        assert!(!rules[0].name.is_empty());
        assert!(!rules[0].url.is_empty());
    }
}
