use crate::geometry::BBox;

/// A single character extracted from a PDF page.
#[derive(Debug, Clone, PartialEq)]
pub struct Char {
    /// The text content of this character.
    pub text: String,
    /// Bounding box in top-left origin coordinates.
    pub bbox: BBox,
    /// Font name.
    pub fontname: String,
    /// Font size in points.
    pub size: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_char_creation() {
        let ch = Char {
            text: "A".to_string(),
            bbox: BBox::new(10.0, 20.0, 20.0, 32.0),
            fontname: "Helvetica".to_string(),
            size: 12.0,
        };
        assert_eq!(ch.text, "A");
        assert_eq!(ch.bbox.x0, 10.0);
        assert_eq!(ch.fontname, "Helvetica");
        assert_eq!(ch.size, 12.0);
    }
}
