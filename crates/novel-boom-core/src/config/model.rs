//! Typed configuration mirrored from `config.toml`.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Root configuration document.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub app: AppSection,
    pub download: DownloadSection,
    pub source: SourceSection,
    pub crawl: CrawlSection,
    pub proxy: ProxySection,
    pub cookie: CookieSection,
}

impl Config {
    /// Validate cross-field constraints after deserialization.
    pub fn validate(&self) -> Result<()> {
        self.download.format.validate()?;
        if self.proxy.enabled && self.proxy.port == 0 {
            return Err(Error::Message(
                "proxy.enabled is true but proxy.port is 0".into(),
            ));
        }
        if self.crawl.max_interval_ms < self.crawl.min_interval_ms {
            return Err(Error::Message(
                "crawl.max_interval_ms must be >= min_interval_ms".into(),
            ));
        }
        if self.crawl.retry_max_interval_ms < self.crawl.retry_min_interval_ms {
            return Err(Error::Message(
                "crawl.retry_max_interval_ms must be >= retry_min_interval_ms".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSection {
    pub auto_update: bool,
    pub gh_proxy: String,
    pub cf_bypass: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DownloadSection {
    pub path: String,
    pub format: DownloadFormat,
    pub txt_encoding: String,
    pub preserve_chapter_cache: bool,
}

impl Default for DownloadSection {
    fn default() -> Self {
        Self {
            path: "downloads".into(),
            format: DownloadFormat::Epub,
            txt_encoding: "UTF-8".into(),
            preserve_chapter_cache: false,
        }
    }
}

/// Supported export formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DownloadFormat {
    Txt,
    #[default]
    Epub,
    Html,
    Pdf,
}

impl DownloadFormat {
    fn validate(self) -> Result<()> {
        // All variants are accepted; PDF may be unimplemented at runtime later.
        Ok(())
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Txt => "txt",
            Self::Epub => "epub",
            Self::Html => "html",
            Self::Pdf => "pdf",
        }
    }
}

impl std::fmt::Display for DownloadFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SourceSection {
    /// Empty means auto-detect from environment / OS locale later.
    pub language: String,
    pub active_rules: String,
    /// `0` means unset (same idea as so-novel `source-id` empty / -1).
    pub source_id: u32,
    /// `0` means unlimited.
    pub search_limit: u32,
    pub search_filter: bool,
}

impl Default for SourceSection {
    fn default() -> Self {
        Self {
            language: String::new(),
            active_rules: "main.json".into(),
            source_id: 0,
            search_limit: 30,
            search_filter: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CrawlSection {
    /// `0` means automatic concurrency.
    pub concurrency: u32,
    pub min_interval_ms: u64,
    pub max_interval_ms: u64,
    pub retry: bool,
    pub max_retries: u32,
    pub retry_min_interval_ms: u64,
    pub retry_max_interval_ms: u64,
}

impl Default for CrawlSection {
    fn default() -> Self {
        Self {
            concurrency: 0,
            min_interval_ms: 200,
            max_interval_ms: 400,
            retry: true,
            max_retries: 3,
            retry_min_interval_ms: 2000,
            retry_max_interval_ms: 4000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProxySection {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
}

impl Default for ProxySection {
    fn default() -> Self {
        Self {
            enabled: false,
            host: "127.0.0.1".into(),
            port: 7890,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CookieSection {
    pub qidian: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        Config::default().validate().unwrap();
    }

    #[test]
    fn rejects_inverted_intervals() {
        let mut cfg = Config::default();
        cfg.crawl.min_interval_ms = 500;
        cfg.crawl.max_interval_ms = 100;
        assert!(cfg.validate().is_err());
    }
}
