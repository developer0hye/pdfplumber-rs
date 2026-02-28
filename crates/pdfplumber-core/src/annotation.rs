//! PDF annotation types.
//!
//! Provides [`Annotation`] and [`AnnotationType`] for representing PDF page
//! annotations such as text notes, links, highlights, and stamps.

use crate::BBox;

/// Common PDF annotation subtypes.
///
/// Covers the most frequently used annotation types defined in PDF 1.7 (Table 169).
/// Unknown or rare subtypes are represented as [`AnnotationType::Other`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AnnotationType {
    /// Text annotation (sticky note).
    Text,
    /// Link annotation (hyperlink or internal navigation).
    Link,
    /// Free text annotation (directly displayed text).
    FreeText,
    /// Highlight markup annotation.
    Highlight,
    /// Underline markup annotation.
    Underline,
    /// Strikeout markup annotation.
    StrikeOut,
    /// Stamp annotation.
    Stamp,
    /// Square annotation (rectangle shape).
    Square,
    /// Circle annotation (ellipse shape).
    Circle,
    /// Ink annotation (freehand drawing).
    Ink,
    /// Popup annotation (associated with another annotation).
    Popup,
    /// Widget annotation (form field).
    Widget,
    /// Other / unknown annotation subtype.
    Other(String),
}

impl AnnotationType {
    /// Parse an annotation type from a PDF /Subtype name.
    pub fn from_subtype(subtype: &str) -> Self {
        match subtype {
            "Text" => Self::Text,
            "Link" => Self::Link,
            "FreeText" => Self::FreeText,
            "Highlight" => Self::Highlight,
            "Underline" => Self::Underline,
            "StrikeOut" => Self::StrikeOut,
            "Stamp" => Self::Stamp,
            "Square" => Self::Square,
            "Circle" => Self::Circle,
            "Ink" => Self::Ink,
            "Popup" => Self::Popup,
            "Widget" => Self::Widget,
            other => Self::Other(other.to_string()),
        }
    }
}

/// A PDF annotation extracted from a page.
///
/// Represents a single annotation with its type, bounding box, and optional
/// metadata fields (contents, author, modification date).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Annotation {
    /// The annotation type (parsed from /Subtype).
    pub annot_type: AnnotationType,
    /// Bounding box of the annotation on the page.
    pub bbox: BBox,
    /// Text contents of the annotation (/Contents entry).
    pub contents: Option<String>,
    /// Author of the annotation (/T entry).
    pub author: Option<String>,
    /// Modification date (/M entry, raw PDF date string).
    pub date: Option<String>,
    /// Raw /Subtype name as it appears in the PDF.
    pub raw_subtype: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotation_type_from_known_subtypes() {
        assert_eq!(AnnotationType::from_subtype("Text"), AnnotationType::Text);
        assert_eq!(AnnotationType::from_subtype("Link"), AnnotationType::Link);
        assert_eq!(
            AnnotationType::from_subtype("FreeText"),
            AnnotationType::FreeText
        );
        assert_eq!(
            AnnotationType::from_subtype("Highlight"),
            AnnotationType::Highlight
        );
        assert_eq!(
            AnnotationType::from_subtype("Underline"),
            AnnotationType::Underline
        );
        assert_eq!(
            AnnotationType::from_subtype("StrikeOut"),
            AnnotationType::StrikeOut
        );
        assert_eq!(AnnotationType::from_subtype("Stamp"), AnnotationType::Stamp);
    }

    #[test]
    fn annotation_type_from_unknown_subtype() {
        assert_eq!(
            AnnotationType::from_subtype("Watermark"),
            AnnotationType::Other("Watermark".to_string())
        );
    }

    #[test]
    fn annotation_with_all_fields() {
        let annot = Annotation {
            annot_type: AnnotationType::Text,
            bbox: BBox::new(100.0, 200.0, 300.0, 250.0),
            contents: Some("A comment".to_string()),
            author: Some("Alice".to_string()),
            date: Some("D:20240101120000".to_string()),
            raw_subtype: "Text".to_string(),
        };
        assert_eq!(annot.annot_type, AnnotationType::Text);
        assert_eq!(annot.contents.as_deref(), Some("A comment"));
        assert_eq!(annot.author.as_deref(), Some("Alice"));
        assert_eq!(annot.date.as_deref(), Some("D:20240101120000"));
        assert_eq!(annot.raw_subtype, "Text");
    }

    #[test]
    fn annotation_with_no_optional_fields() {
        let annot = Annotation {
            annot_type: AnnotationType::Link,
            bbox: BBox::new(0.0, 0.0, 100.0, 20.0),
            contents: None,
            author: None,
            date: None,
            raw_subtype: "Link".to_string(),
        };
        assert_eq!(annot.annot_type, AnnotationType::Link);
        assert!(annot.contents.is_none());
        assert!(annot.author.is_none());
        assert!(annot.date.is_none());
    }

    #[test]
    fn annotation_clone_and_eq() {
        let annot1 = Annotation {
            annot_type: AnnotationType::Highlight,
            bbox: BBox::new(10.0, 20.0, 30.0, 40.0),
            contents: Some("highlighted".to_string()),
            author: None,
            date: None,
            raw_subtype: "Highlight".to_string(),
        };
        let annot2 = annot1.clone();
        assert_eq!(annot1, annot2);
    }
}
