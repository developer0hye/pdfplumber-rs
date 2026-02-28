//! PDF bookmark / outline / table of contents types.
//!
//! Provides [`Bookmark`] for representing entries in the PDF document outline
//! tree (bookmarks / table of contents).

/// A single entry in the PDF document outline (bookmark / table of contents).
///
/// Bookmarks are extracted from the `/Outlines` dictionary in the PDF catalog.
/// Each bookmark has a title, a nesting level, and optionally a destination
/// page number and y-coordinate.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bookmark {
    /// The bookmark title text.
    pub title: String,
    /// Nesting depth (0-indexed). Top-level bookmarks have level 0.
    pub level: usize,
    /// The 0-indexed destination page number, if resolvable.
    pub page_number: Option<usize>,
    /// The y-coordinate on the destination page (top of view), if available.
    pub dest_top: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookmark_with_all_fields() {
        let bm = Bookmark {
            title: "Chapter 1".to_string(),
            level: 0,
            page_number: Some(0),
            dest_top: Some(792.0),
        };
        assert_eq!(bm.title, "Chapter 1");
        assert_eq!(bm.level, 0);
        assert_eq!(bm.page_number, Some(0));
        assert_eq!(bm.dest_top, Some(792.0));
    }

    #[test]
    fn bookmark_without_destination() {
        let bm = Bookmark {
            title: "Appendix".to_string(),
            level: 1,
            page_number: None,
            dest_top: None,
        };
        assert_eq!(bm.title, "Appendix");
        assert_eq!(bm.level, 1);
        assert!(bm.page_number.is_none());
        assert!(bm.dest_top.is_none());
    }

    #[test]
    fn bookmark_clone_and_eq() {
        let bm1 = Bookmark {
            title: "Section 2.1".to_string(),
            level: 2,
            page_number: Some(5),
            dest_top: Some(500.0),
        };
        let bm2 = bm1.clone();
        assert_eq!(bm1, bm2);
    }

    #[test]
    fn bookmark_nested_levels() {
        let bookmarks = vec![
            Bookmark {
                title: "Chapter 1".to_string(),
                level: 0,
                page_number: Some(0),
                dest_top: None,
            },
            Bookmark {
                title: "Section 1.1".to_string(),
                level: 1,
                page_number: Some(2),
                dest_top: None,
            },
            Bookmark {
                title: "Section 1.1.1".to_string(),
                level: 2,
                page_number: Some(3),
                dest_top: None,
            },
            Bookmark {
                title: "Chapter 2".to_string(),
                level: 0,
                page_number: Some(10),
                dest_top: None,
            },
        ];
        assert_eq!(bookmarks.len(), 4);
        assert_eq!(bookmarks[0].level, 0);
        assert_eq!(bookmarks[1].level, 1);
        assert_eq!(bookmarks[2].level, 2);
        assert_eq!(bookmarks[3].level, 0);
    }
}
