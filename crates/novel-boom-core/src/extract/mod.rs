//! HTML field extraction (CSS selectors; XPath/JS later).

mod query;
mod select;

pub use query::{ContentKind, content_kind, requires_js, requires_xpath, strip_extensions};
pub use select::{
    extract, extract_content, extract_in_document, select_all, select_all_in_document,
    select_first,
};
