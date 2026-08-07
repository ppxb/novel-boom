//! Pure domain models (no I/O).

mod book;
mod chapter;
mod search;

pub use book::Book;
pub use chapter::Chapter;
pub use search::SearchHit;
