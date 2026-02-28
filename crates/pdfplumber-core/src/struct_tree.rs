//! PDF structure tree types for tagged PDF access.
//!
//! Tagged PDFs contain a logical structure tree that describes the document's
//! semantic structure (headings, paragraphs, tables, lists). This module provides
//! the [`StructElement`] type for representing structure tree nodes.

use crate::geometry::BBox;

/// A node in the PDF structure tree.
///
/// Represents a logical structure element from a tagged PDF's `/StructTreeRoot`.
/// Each element has a type (e.g., "H1", "P", "Table"), optional marked content
/// identifiers (MCIDs) linking it to page content, and optional child elements
/// forming a tree structure.
///
/// # Tagged PDF Support
///
/// Tagged PDFs (ISO 32000-1, Section 14.8) embed semantic structure that is
/// critical for accessibility and increasingly important for AI/LLM document
/// understanding. The structure tree maps logical elements (headings, paragraphs,
/// tables) to their visual representation on the page via MCID references.
///
/// # Example
///
/// ```
/// use pdfplumber_core::StructElement;
///
/// let heading = StructElement {
///     element_type: "H1".to_string(),
///     mcids: vec![0],
///     alt_text: None,
///     actual_text: Some("Chapter 1".to_string()),
///     lang: Some("en".to_string()),
///     bbox: None,
///     children: vec![],
///     page_index: Some(0),
/// };
/// assert_eq!(heading.element_type, "H1");
/// assert_eq!(heading.mcids, vec![0]);
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StructElement {
    /// The structure type name (e.g., "Document", "H1", "P", "Table", "TR", "TD",
    /// "L", "LI", "Span", "Figure").
    pub element_type: String,
    /// Marked content identifiers linking this element to page content.
    /// Each MCID corresponds to a marked-content sequence in a page's content stream.
    pub mcids: Vec<u32>,
    /// Alternative text for accessibility (from `/Alt` entry).
    pub alt_text: Option<String>,
    /// Replacement text for the element's content (from `/ActualText` entry).
    pub actual_text: Option<String>,
    /// Language of the element's content (from `/Lang` entry, e.g., "en-US").
    pub lang: Option<String>,
    /// Bounding box of the element, if available.
    pub bbox: Option<BBox>,
    /// Child structure elements forming the tree hierarchy.
    pub children: Vec<StructElement>,
    /// Page index (0-based) this element belongs to, if determinable.
    pub page_index: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_element_basic_creation() {
        let elem = StructElement {
            element_type: "P".to_string(),
            mcids: vec![1, 2],
            alt_text: None,
            actual_text: None,
            lang: None,
            bbox: None,
            children: vec![],
            page_index: Some(0),
        };
        assert_eq!(elem.element_type, "P");
        assert_eq!(elem.mcids, vec![1, 2]);
        assert!(elem.alt_text.is_none());
        assert!(elem.actual_text.is_none());
        assert!(elem.lang.is_none());
        assert!(elem.bbox.is_none());
        assert!(elem.children.is_empty());
        assert_eq!(elem.page_index, Some(0));
    }

    #[test]
    fn struct_element_with_all_fields() {
        let elem = StructElement {
            element_type: "Figure".to_string(),
            mcids: vec![5],
            alt_text: Some("A bar chart showing quarterly revenue".to_string()),
            actual_text: Some("Revenue chart".to_string()),
            lang: Some("en-US".to_string()),
            bbox: Some(BBox::new(72.0, 100.0, 540.0, 400.0)),
            children: vec![],
            page_index: Some(2),
        };
        assert_eq!(elem.element_type, "Figure");
        assert_eq!(
            elem.alt_text.as_deref(),
            Some("A bar chart showing quarterly revenue")
        );
        assert_eq!(elem.actual_text.as_deref(), Some("Revenue chart"));
        assert_eq!(elem.lang.as_deref(), Some("en-US"));
        assert!(elem.bbox.is_some());
        assert_eq!(elem.page_index, Some(2));
    }

    #[test]
    fn struct_element_with_children() {
        let child1 = StructElement {
            element_type: "Span".to_string(),
            mcids: vec![1],
            alt_text: None,
            actual_text: None,
            lang: None,
            bbox: None,
            children: vec![],
            page_index: Some(0),
        };
        let child2 = StructElement {
            element_type: "Span".to_string(),
            mcids: vec![2],
            alt_text: None,
            actual_text: None,
            lang: None,
            bbox: None,
            children: vec![],
            page_index: Some(0),
        };
        let parent = StructElement {
            element_type: "P".to_string(),
            mcids: vec![],
            alt_text: None,
            actual_text: None,
            lang: None,
            bbox: None,
            children: vec![child1, child2],
            page_index: Some(0),
        };
        assert_eq!(parent.children.len(), 2);
        assert_eq!(parent.children[0].element_type, "Span");
        assert_eq!(parent.children[1].mcids, vec![2]);
    }

    #[test]
    fn struct_element_nested_tree() {
        let td1 = StructElement {
            element_type: "TD".to_string(),
            mcids: vec![10],
            alt_text: None,
            actual_text: None,
            lang: None,
            bbox: None,
            children: vec![],
            page_index: Some(0),
        };
        let td2 = StructElement {
            element_type: "TD".to_string(),
            mcids: vec![11],
            alt_text: None,
            actual_text: None,
            lang: None,
            bbox: None,
            children: vec![],
            page_index: Some(0),
        };
        let tr = StructElement {
            element_type: "TR".to_string(),
            mcids: vec![],
            alt_text: None,
            actual_text: None,
            lang: None,
            bbox: None,
            children: vec![td1, td2],
            page_index: Some(0),
        };
        let table = StructElement {
            element_type: "Table".to_string(),
            mcids: vec![],
            alt_text: None,
            actual_text: None,
            lang: None,
            bbox: None,
            children: vec![tr],
            page_index: Some(0),
        };

        assert_eq!(table.children.len(), 1);
        assert_eq!(table.children[0].element_type, "TR");
        assert_eq!(table.children[0].children.len(), 2);
        assert_eq!(table.children[0].children[0].element_type, "TD");
        assert_eq!(table.children[0].children[0].mcids, vec![10]);
    }

    #[test]
    fn struct_element_clone() {
        let elem = StructElement {
            element_type: "H1".to_string(),
            mcids: vec![0],
            alt_text: Some("Title".to_string()),
            actual_text: None,
            lang: Some("en".to_string()),
            bbox: Some(BBox::new(72.0, 72.0, 540.0, 100.0)),
            children: vec![],
            page_index: Some(0),
        };
        let cloned = elem.clone();
        assert_eq!(elem, cloned);
    }

    #[test]
    fn struct_element_no_page_index() {
        let elem = StructElement {
            element_type: "Document".to_string(),
            mcids: vec![],
            alt_text: None,
            actual_text: None,
            lang: None,
            bbox: None,
            children: vec![],
            page_index: None,
        };
        assert!(elem.page_index.is_none());
    }

    #[test]
    fn struct_element_empty_mcids() {
        let elem = StructElement {
            element_type: "Div".to_string(),
            mcids: vec![],
            alt_text: None,
            actual_text: None,
            lang: None,
            bbox: None,
            children: vec![],
            page_index: None,
        };
        assert!(elem.mcids.is_empty());
    }

    #[test]
    fn struct_element_heading_types() {
        for level in 1..=6 {
            let elem = StructElement {
                element_type: format!("H{level}"),
                mcids: vec![level as u32],
                alt_text: None,
                actual_text: None,
                lang: None,
                bbox: None,
                children: vec![],
                page_index: Some(0),
            };
            assert_eq!(elem.element_type, format!("H{level}"));
        }
    }
}
