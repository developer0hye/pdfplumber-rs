//! Document-level metadata types.
//!
//! Provides [`DocumentMetadata`] for PDF document information dictionary fields
//! such as title, author, creation date, etc.

/// Document-level metadata extracted from the PDF /Info dictionary.
///
/// All fields are optional since PDFs may omit the /Info dictionary entirely
/// or include only a subset of fields.
///
/// # PDF Date Format
///
/// Date fields (`creation_date`, `mod_date`) are stored as raw PDF date
/// strings in the format `D:YYYYMMDDHHmmSSOHH'mm'`. Use
/// [`DocumentMetadata::parse_pdf_date`] to extract components.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DocumentMetadata {
    /// Document title.
    pub title: Option<String>,
    /// Document author.
    pub author: Option<String>,
    /// Document subject / description.
    pub subject: Option<String>,
    /// Keywords associated with the document.
    pub keywords: Option<String>,
    /// Application that created the original document.
    pub creator: Option<String>,
    /// Application that produced the PDF.
    pub producer: Option<String>,
    /// Date the document was created (raw PDF date string).
    pub creation_date: Option<String>,
    /// Date the document was last modified (raw PDF date string).
    pub mod_date: Option<String>,
}

impl DocumentMetadata {
    /// Returns `true` if all metadata fields are `None`.
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.author.is_none()
            && self.subject.is_none()
            && self.keywords.is_none()
            && self.creator.is_none()
            && self.producer.is_none()
            && self.creation_date.is_none()
            && self.mod_date.is_none()
    }
}

/// A recursively decoded value from the PDF document information dictionary.
///
/// Dictionary entries use a vector rather than a map so callers can retain the
/// source order exposed by Python pdfplumber.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MetadataValue {
    /// The PDF `null` value.
    Null,
    /// A PDF boolean.
    Boolean(bool),
    /// A PDF integer.
    Integer(i64),
    /// A PDF real number.
    Real(f64),
    /// A decoded PDF string or name.
    String(String),
    /// A recursively decoded PDF array.
    Array(Vec<MetadataValue>),
    /// A recursively decoded PDF dictionary in source order.
    Dictionary(Vec<(String, MetadataValue)>),
    /// An indirect reference retained because its value could not be resolved.
    Reference(MetadataReference),
    /// A PDF stream value that is not decoded as document metadata text.
    Stream {
        /// The recursively decoded stream dictionary.
        dictionary: Vec<(String, MetadataValue)>,
        /// The raw stream bytes.
        data: Vec<u8>,
    },
}

/// The object identifier for an unresolved metadata reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MetadataReference {
    /// The indirect object's number.
    pub object_number: u32,
    /// The indirect object's generation number.
    pub generation_number: u16,
}

/// One entry in the raw document information dictionary.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MetadataEntry {
    /// The decoded metadata key.
    pub key: String,
    /// The decoded value, or the original unresolved value after a failure.
    pub value: MetadataValue,
    /// The resolution failure reported for this entry, if any.
    pub resolution_error: Option<String>,
}

/// The complete source-ordered document information dictionary.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RawDocumentMetadata {
    /// Information-dictionary entries in source order.
    pub entries: Vec<MetadataEntry>,
}

impl RawDocumentMetadata {
    /// Returns `true` if the information dictionary has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_metadata_is_empty() {
        let meta = DocumentMetadata::default();
        assert!(meta.is_empty());
        assert_eq!(meta.title, None);
        assert_eq!(meta.author, None);
        assert_eq!(meta.subject, None);
        assert_eq!(meta.keywords, None);
        assert_eq!(meta.creator, None);
        assert_eq!(meta.producer, None);
        assert_eq!(meta.creation_date, None);
        assert_eq!(meta.mod_date, None);
    }

    #[test]
    fn metadata_with_all_fields() {
        let meta = DocumentMetadata {
            title: Some("Test Document".to_string()),
            author: Some("John Doe".to_string()),
            subject: Some("Testing".to_string()),
            keywords: Some("test, pdf, rust".to_string()),
            creator: Some("LibreOffice".to_string()),
            producer: Some("pdfplumber-rs".to_string()),
            creation_date: Some("D:20240101120000+00'00'".to_string()),
            mod_date: Some("D:20240615153000+00'00'".to_string()),
        };
        assert!(!meta.is_empty());
        assert_eq!(meta.title.as_deref(), Some("Test Document"));
        assert_eq!(meta.author.as_deref(), Some("John Doe"));
    }

    #[test]
    fn metadata_with_partial_fields() {
        let meta = DocumentMetadata {
            title: Some("Only Title".to_string()),
            ..Default::default()
        };
        assert!(!meta.is_empty());
        assert_eq!(meta.title.as_deref(), Some("Only Title"));
        assert_eq!(meta.author, None);
    }

    #[test]
    fn metadata_clone_and_eq() {
        let meta1 = DocumentMetadata {
            title: Some("Test".to_string()),
            ..Default::default()
        };
        let meta2 = meta1.clone();
        assert_eq!(meta1, meta2);
    }
}
