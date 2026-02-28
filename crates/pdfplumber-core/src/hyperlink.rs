//! PDF hyperlink types.
//!
//! Provides [`Hyperlink`] for representing resolved URI links extracted from
//! PDF Link annotations.

use crate::BBox;

/// A resolved hyperlink extracted from a PDF page.
///
/// Represents a Link annotation with its bounding box and resolved URI target.
/// Created by filtering annotations with `/Subtype /Link` and resolving their
/// `/A` (action) or `/Dest` entries.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Hyperlink {
    /// Bounding box of the link on the page.
    pub bbox: BBox,
    /// The resolved URI or destination string.
    pub uri: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hyperlink_with_uri() {
        let link = Hyperlink {
            bbox: BBox::new(100.0, 200.0, 300.0, 220.0),
            uri: "https://example.com".to_string(),
        };
        assert_eq!(link.uri, "https://example.com");
        assert_eq!(link.bbox.x0, 100.0);
    }

    #[test]
    fn hyperlink_with_goto_dest() {
        let link = Hyperlink {
            bbox: BBox::new(50.0, 100.0, 150.0, 120.0),
            uri: "#page=3".to_string(),
        };
        assert_eq!(link.uri, "#page=3");
    }

    #[test]
    fn hyperlink_clone_and_eq() {
        let link1 = Hyperlink {
            bbox: BBox::new(10.0, 20.0, 30.0, 40.0),
            uri: "https://rust-lang.org".to_string(),
        };
        let link2 = link1.clone();
        assert_eq!(link1, link2);
    }
}
