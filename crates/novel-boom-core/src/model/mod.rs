//! Pure domain models (no I/O).

mod book;
mod chapter;
mod range;
mod search;

pub use book::Book;
pub use chapter::Chapter;
pub use range::{ChapterRange, apply_chapter_range};
pub use search::SearchHit;
