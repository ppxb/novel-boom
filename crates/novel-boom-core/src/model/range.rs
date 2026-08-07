//! Chapter range selection helpers.

use crate::error::{Error, Result};
use crate::model::Chapter;

/// How the user wants to slice a full table of contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChapterRange {
    /// Entire TOC.
    All,
    /// Inclusive 1-based chapter indexes.
    Span { start: u32, end: u32 },
    /// Last N chapters.
    Latest { count: u32 },
}

/// Apply a range to a full ordered TOC and renumber `order` from 1.
pub fn apply_chapter_range(chapters: &[Chapter], range: ChapterRange) -> Result<Vec<Chapter>> {
    if chapters.is_empty() {
        return Err(Error::Message("目录为空，无法选择章节范围".into()));
    }

    let total = chapters.len();
    let slice: &[Chapter] = match range {
        ChapterRange::All => chapters,
        ChapterRange::Span { start, end } => {
            if start == 0 || end == 0 || start > end {
                return Err(Error::Message(
                    "章节范围无效：起始章与结束章须为从 1 开始的正整数，且起始 ≤ 结束".into(),
                ));
            }
            if end as usize > total {
                return Err(Error::Message(format!(
                    "章节范围超出目录：共 {total} 章，请求 {start}-{end}"
                )));
            }
            &chapters[(start as usize - 1)..(end as usize)]
        }
        ChapterRange::Latest { count } => {
            if count == 0 {
                return Err(Error::Message("最新章节数量须 ≥ 1".into()));
            }
            let count = (count as usize).min(total);
            &chapters[total - count..]
        }
    };

    Ok(slice
        .iter()
        .cloned()
        .enumerate()
        .map(|(idx, mut ch)| {
            ch.order = (idx + 1) as u32;
            ch
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<Chapter> {
        (1..=5)
            .map(|i| Chapter {
                order: i,
                title: format!("第{i}章"),
                url: format!("http://x/{i}"),
                content: String::new(),
            })
            .collect()
    }

    #[test]
    fn span_and_latest() {
        let toc = sample();
        let mid = apply_chapter_range(&toc, ChapterRange::Span { start: 2, end: 4 }).unwrap();
        assert_eq!(mid.len(), 3);
        assert_eq!(mid[0].title, "第2章");
        assert_eq!(mid[0].order, 1);

        let latest = apply_chapter_range(&toc, ChapterRange::Latest { count: 2 }).unwrap();
        assert_eq!(latest[0].title, "第4章");
        assert_eq!(latest[1].title, "第5章");
    }
}
