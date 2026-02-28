//! PDF form field types for AcroForm extraction.
//!
//! Provides [`FormField`] and [`FieldType`] for representing PDF interactive
//! form fields (AcroForms) such as text inputs, checkboxes, dropdowns, and
//! signature fields.

use crate::BBox;

/// The type of a PDF form field.
///
/// Corresponds to the `/FT` entry in a field dictionary (PDF 1.7 Table 220).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FieldType {
    /// Text field (`/FT /Tx`) — accepts text input.
    Text,
    /// Button field (`/FT /Btn`) — checkboxes, radio buttons, push buttons.
    Button,
    /// Choice field (`/FT /Ch`) — dropdowns, list boxes.
    Choice,
    /// Signature field (`/FT /Sig`) — digital signature.
    Signature,
}

impl FieldType {
    /// Parse a field type from its PDF name string.
    ///
    /// Returns `None` if the string is not a recognized field type.
    pub fn from_pdf_name(name: &str) -> Option<Self> {
        match name {
            "Tx" => Some(Self::Text),
            "Btn" => Some(Self::Button),
            "Ch" => Some(Self::Choice),
            "Sig" => Some(Self::Signature),
            _ => None,
        }
    }

    /// Return the PDF name string for this field type.
    pub fn as_pdf_name(&self) -> &'static str {
        match self {
            Self::Text => "Tx",
            Self::Button => "Btn",
            Self::Choice => "Ch",
            Self::Signature => "Sig",
        }
    }
}

impl std::fmt::Display for FieldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text => write!(f, "Text"),
            Self::Button => write!(f, "Button"),
            Self::Choice => write!(f, "Choice"),
            Self::Signature => write!(f, "Signature"),
        }
    }
}

/// A PDF form field extracted from the document's AcroForm dictionary.
///
/// Represents a single interactive form field with its name, type, value,
/// and visual position on the page.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FormField {
    /// Field name from `/T` entry. Hierarchical fields join names with `.`.
    pub name: String,
    /// Field type from `/FT` entry.
    pub field_type: FieldType,
    /// Current value from `/V` entry.
    pub value: Option<String>,
    /// Default value from `/DV` entry.
    pub default_value: Option<String>,
    /// Bounding box from `/Rect` entry.
    pub bbox: BBox,
    /// Options for choice fields from `/Opt` entry.
    pub options: Vec<String>,
    /// Field flags from `/Ff` entry (bitmask).
    pub flags: u32,
    /// The 0-based page index this field belongs to, if determinable.
    pub page_index: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_type_from_pdf_name_text() {
        assert_eq!(FieldType::from_pdf_name("Tx"), Some(FieldType::Text));
    }

    #[test]
    fn field_type_from_pdf_name_button() {
        assert_eq!(FieldType::from_pdf_name("Btn"), Some(FieldType::Button));
    }

    #[test]
    fn field_type_from_pdf_name_choice() {
        assert_eq!(FieldType::from_pdf_name("Ch"), Some(FieldType::Choice));
    }

    #[test]
    fn field_type_from_pdf_name_signature() {
        assert_eq!(FieldType::from_pdf_name("Sig"), Some(FieldType::Signature));
    }

    #[test]
    fn field_type_from_pdf_name_unknown() {
        assert_eq!(FieldType::from_pdf_name("Unknown"), None);
    }

    #[test]
    fn field_type_as_pdf_name() {
        assert_eq!(FieldType::Text.as_pdf_name(), "Tx");
        assert_eq!(FieldType::Button.as_pdf_name(), "Btn");
        assert_eq!(FieldType::Choice.as_pdf_name(), "Ch");
        assert_eq!(FieldType::Signature.as_pdf_name(), "Sig");
    }

    #[test]
    fn field_type_display() {
        assert_eq!(format!("{}", FieldType::Text), "Text");
        assert_eq!(format!("{}", FieldType::Button), "Button");
        assert_eq!(format!("{}", FieldType::Choice), "Choice");
        assert_eq!(format!("{}", FieldType::Signature), "Signature");
    }

    #[test]
    fn form_field_text_with_value() {
        let field = FormField {
            name: "full_name".to_string(),
            field_type: FieldType::Text,
            value: Some("John Doe".to_string()),
            default_value: None,
            bbox: BBox::new(50.0, 100.0, 200.0, 120.0),
            options: vec![],
            flags: 0,
            page_index: Some(0),
        };
        assert_eq!(field.name, "full_name");
        assert_eq!(field.field_type, FieldType::Text);
        assert_eq!(field.value.as_deref(), Some("John Doe"));
        assert!(field.default_value.is_none());
        assert!(field.options.is_empty());
        assert_eq!(field.flags, 0);
        assert_eq!(field.page_index, Some(0));
    }

    #[test]
    fn form_field_checkbox() {
        let field = FormField {
            name: "agree".to_string(),
            field_type: FieldType::Button,
            value: Some("Yes".to_string()),
            default_value: Some("Off".to_string()),
            bbox: BBox::new(30.0, 200.0, 50.0, 220.0),
            options: vec![],
            flags: 0,
            page_index: Some(0),
        };
        assert_eq!(field.field_type, FieldType::Button);
        assert_eq!(field.value.as_deref(), Some("Yes"));
        assert_eq!(field.default_value.as_deref(), Some("Off"));
    }

    #[test]
    fn form_field_dropdown_with_options() {
        let field = FormField {
            name: "country".to_string(),
            field_type: FieldType::Choice,
            value: Some("US".to_string()),
            default_value: None,
            bbox: BBox::new(50.0, 300.0, 200.0, 320.0),
            options: vec!["US".to_string(), "UK".to_string(), "FR".to_string()],
            flags: 0,
            page_index: Some(0),
        };
        assert_eq!(field.field_type, FieldType::Choice);
        assert_eq!(field.options.len(), 3);
        assert_eq!(field.options[0], "US");
        assert_eq!(field.options[2], "FR");
    }

    #[test]
    fn form_field_with_no_value() {
        let field = FormField {
            name: "email".to_string(),
            field_type: FieldType::Text,
            value: None,
            default_value: None,
            bbox: BBox::new(50.0, 400.0, 200.0, 420.0),
            options: vec![],
            flags: 0,
            page_index: None,
        };
        assert!(field.value.is_none());
        assert!(field.page_index.is_none());
    }

    #[test]
    fn form_field_signature() {
        let field = FormField {
            name: "signature".to_string(),
            field_type: FieldType::Signature,
            value: None,
            default_value: None,
            bbox: BBox::new(100.0, 500.0, 300.0, 600.0),
            options: vec![],
            flags: 0,
            page_index: Some(1),
        };
        assert_eq!(field.field_type, FieldType::Signature);
    }

    #[test]
    fn form_field_with_flags() {
        let field = FormField {
            name: "readonly_field".to_string(),
            field_type: FieldType::Text,
            value: Some("Cannot edit".to_string()),
            default_value: None,
            bbox: BBox::new(50.0, 100.0, 200.0, 120.0),
            options: vec![],
            flags: 1, // ReadOnly
            page_index: Some(0),
        };
        assert_eq!(field.flags, 1);
    }

    #[test]
    fn form_field_clone_and_eq() {
        let field1 = FormField {
            name: "test".to_string(),
            field_type: FieldType::Text,
            value: Some("val".to_string()),
            default_value: None,
            bbox: BBox::new(0.0, 0.0, 100.0, 20.0),
            options: vec![],
            flags: 0,
            page_index: Some(0),
        };
        let field2 = field1.clone();
        assert_eq!(field1, field2);
    }
}
