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
