//! PDF digital signature information types.
//!
//! Provides [`SignatureInfo`] for extracting metadata from PDF signature
//! fields. This is metadata extraction only — not signature verification.

/// Digital signature metadata extracted from a PDF signature field.
///
/// Represents the metadata from a `/FT /Sig` form field and its
/// signature value dictionary (`/V`). When `is_signed` is `false`,
/// the field exists but has no signature value.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SignatureInfo {
    /// Signer name from the `/Name` entry in the signature dictionary.
    pub signer_name: Option<String>,
    /// Signing date from the `/M` entry (PDF date format).
    pub sign_date: Option<String>,
    /// Reason for signing from the `/Reason` entry.
    pub reason: Option<String>,
    /// Location of signing from the `/Location` entry.
    pub location: Option<String>,
    /// Contact information from the `/ContactInfo` entry.
    pub contact_info: Option<String>,
    /// Whether this signature field has been signed (has a `/V` value).
    pub is_signed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_info_signed() {
        let sig = SignatureInfo {
            signer_name: Some("John Doe".to_string()),
            sign_date: Some("D:20260228120000+00'00'".to_string()),
            reason: Some("Approval".to_string()),
            location: Some("Seoul".to_string()),
            contact_info: Some("john@example.com".to_string()),
            is_signed: true,
        };
        assert!(sig.is_signed);
        assert_eq!(sig.signer_name.as_deref(), Some("John Doe"));
        assert_eq!(sig.reason.as_deref(), Some("Approval"));
        assert_eq!(sig.location.as_deref(), Some("Seoul"));
        assert_eq!(sig.contact_info.as_deref(), Some("john@example.com"));
    }

    #[test]
    fn signature_info_unsigned() {
        let sig = SignatureInfo {
            signer_name: None,
            sign_date: None,
            reason: None,
            location: None,
            contact_info: None,
            is_signed: false,
        };
        assert!(!sig.is_signed);
        assert!(sig.signer_name.is_none());
        assert!(sig.sign_date.is_none());
    }

    #[test]
    fn signature_info_partial_metadata() {
        let sig = SignatureInfo {
            signer_name: Some("Jane".to_string()),
            sign_date: None,
            reason: None,
            location: Some("NYC".to_string()),
            contact_info: None,
            is_signed: true,
        };
        assert!(sig.is_signed);
        assert_eq!(sig.signer_name.as_deref(), Some("Jane"));
        assert!(sig.sign_date.is_none());
        assert!(sig.reason.is_none());
        assert_eq!(sig.location.as_deref(), Some("NYC"));
    }

    #[test]
    fn signature_info_clone_and_eq() {
        let sig1 = SignatureInfo {
            signer_name: Some("Test".to_string()),
            sign_date: Some("D:20260101".to_string()),
            reason: None,
            location: None,
            contact_info: None,
            is_signed: true,
        };
        let sig2 = sig1.clone();
        assert_eq!(sig1, sig2);
    }
}
