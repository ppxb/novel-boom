//! Load and resolve configuration files.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

use super::model::Config;

/// Default config file name looked up next to the executable / CWD.
pub const DEFAULT_CONFIG_FILE: &str = "config.toml";

/// Resolve which path to load.
///
/// Order:
/// 1. Explicit `override_path` if provided
/// 2. `NOVEL_BOOM_CONFIG` environment variable
/// 3. `./config.toml`
pub fn resolve_config_path(override_path: Option<&Path>) -> PathBuf {
    if let Some(path) = override_path {
        return path.to_path_buf();
    }
    if let Ok(from_env) = std::env::var("NOVEL_BOOM_CONFIG") {
        if !from_env.trim().is_empty() {
            return PathBuf::from(from_env);
        }
    }
    PathBuf::from(DEFAULT_CONFIG_FILE)
}

/// Load config from `path`. Fails if the file is missing or invalid.
pub fn load(path: impl AsRef<Path>) -> Result<Config> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(Error::ConfigNotFound(path.to_path_buf()));
    }

    let raw = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&raw).map_err(|err| Error::InvalidConfig {
        path: path.to_path_buf(),
        source: Box::new(err),
    })?;
    config.validate()?;
    Ok(config)
}

/// Load config if present; otherwise return defaults.
///
/// Missing file is not an error (first-run friendly). Invalid file still errors.
pub fn load_or_default(path: impl AsRef<Path>) -> Result<Config> {
    let path = path.as_ref();
    if !path.exists() {
        tracing::warn!(
            path = %path.display(),
            "config file not found; using built-in defaults"
        );
        let config = Config::default();
        config.validate()?;
        return Ok(config);
    }
    load(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn load_example_shaped_toml() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        write!(
            file,
            r#"
[download]
path = "out"
format = "txt"

[source]
active_rules = "main.json"
search_limit = 10
"#
        )
        .unwrap();

        let cfg = load(file.path()).unwrap();
        assert_eq!(cfg.download.path, "out");
        assert_eq!(cfg.download.format.as_str(), "txt");
        assert_eq!(cfg.source.active_rules, "main.json");
        assert_eq!(cfg.source.search_limit, 10);
        // Unspecified sections keep defaults.
        assert_eq!(cfg.crawl.min_interval_ms, 200);
    }

    #[test]
    fn load_or_default_missing_file() {
        let cfg = load_or_default("definitely-missing-novel-boom-config.toml").unwrap();
        assert_eq!(cfg.source.active_rules, "main.json");
    }
}
