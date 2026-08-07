//! Application configuration (TOML).

mod load;
mod model;

pub use load::{load, load_or_default, resolve_config_path};
pub use model::{
    AppSection, Config, CookieSection, CrawlSection, DownloadFormat, DownloadSection, ProxySection,
    SourceSection,
};
