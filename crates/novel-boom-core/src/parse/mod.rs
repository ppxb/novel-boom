//! Page parsers that combine HTTP + extraction rules.

mod form_data;
mod search;

pub use form_data::parse_form_template;
pub use search::{parse_search_html, search_source};
