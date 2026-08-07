//! Application use-cases consumed by TUI/CLI.

mod book;
mod search;

pub use book::{fetch_book_catalog, select_chapters, BookCatalog};
pub use search::single_search;
