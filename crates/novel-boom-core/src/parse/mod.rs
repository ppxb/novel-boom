//! Page parsers that combine HTTP + extraction rules.

mod book;
mod form_data;
mod search;
mod toc;

pub use book::{fetch_book, parse_book_html};
pub use form_data::parse_form_template;
pub use search::{parse_search_html, search_source};
pub use toc::{fetch_toc, parse_toc_html};
