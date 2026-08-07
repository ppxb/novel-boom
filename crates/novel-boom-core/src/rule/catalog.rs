//! Derived catalog views over loaded rules.

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::Result;

use super::load::{load_active_rules, resolve_rules_path};
use super::model::Rule;

/// Lightweight row for UI / listings (no full nested selectors).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInfo {
    pub id: u32,
    pub name: String,
    pub url: String,
    pub comment: String,
    pub disabled: bool,
    pub searchable: bool,
    pub need_proxy: bool,
}

impl From<&Rule> for SourceInfo {
    fn from(rule: &Rule) -> Self {
        Self {
            id: rule.id,
            name: rule.name.clone(),
            url: rule.url.clone(),
            comment: rule.comment.clone(),
            disabled: rule.disabled,
            searchable: rule.is_searchable(),
            need_proxy: rule.need_proxy,
        }
    }
}

/// Active rules pack plus resolved path metadata.
#[derive(Debug, Clone)]
pub struct RuleCatalog {
    /// Absolute or normalized path that was loaded.
    pub path: PathBuf,
    pub rules: Vec<Rule>,
}

impl RuleCatalog {
    /// Load catalog for the given config.
    pub fn load(config: &Config, config_path: Option<&Path>) -> Result<Self> {
        let path = resolve_rules_path(config, config_path);
        let rules = load_active_rules(config, config_path)?;
        Ok(Self { path, rules })
    }

    /// Compact rows for display.
    pub fn sources(&self) -> Vec<SourceInfo> {
        self.rules.iter().map(SourceInfo::from).collect()
    }

    /// Sources that can be used for keyword search.
    pub fn searchable_sources(&self) -> Vec<SourceInfo> {
        self.rules
            .iter()
            .filter(|r| r.is_searchable())
            .map(SourceInfo::from)
            .collect()
    }

    /// Look up a rule by 1-based id.
    pub fn get(&self, source_id: u32) -> Option<&Rule> {
        self.rules.iter().find(|r| r.id == source_id)
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn searchable_count(&self) -> usize {
        self.rules.iter().filter(|r| r.is_searchable()).count()
    }

    pub fn enabled_count(&self) -> usize {
        self.rules.iter().filter(|r| !r.disabled).count()
    }
}
