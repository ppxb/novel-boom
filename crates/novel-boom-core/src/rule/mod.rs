//! Book source rules (so-novel JSON packs).

mod catalog;
mod defaults;
mod load;
mod model;

pub use catalog::{RuleCatalog, SourceInfo};
pub use defaults::{effective_book_rule, effective_search_rule, effective_toc_rule};
pub use load::{
    DEFAULT_RULES_DIR, load_active_rules, load_rules_file, resolve_rules_path,
};
pub use model::{
    BookRule, ChapterRule, CrawlRule, Rule, SearchRule, TocRule,
};
