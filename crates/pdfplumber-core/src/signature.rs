//! PDF digital signature types.
//!
//! Two levels of detail are provided:
//!
//! - [`SignatureInfo`]: metadata only (name, date, reason). Always available,
//!   no crypto. Produced by the base extraction layer.
//!
//! - [`SignatureVerification`]: cryptographic verification result. Only
//!   produced when the `signatures` feature is enabled on the `pdfplumber`
//!   crate. Carries `is_valid`, trust chain, coverage, etc.
//!
//! [`RawSignature`]: internal type that carries the raw bytes needed for
//! verification — extracted from lopdf but not publicly exported from core
//! (only used in `pdfplumber`'s signatures module).

/// Digital signature metadata extracted from a PDF `/FT /Sig` form field.
///
/// This is the always-available view. It contains whatever the signer wrote
/// into the visible metadata fields — name, date, reason, location. It does
/// NOT verify anything. Use [`SignatureVerification`] for that.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SignatureInfo {
    /// Signer name from `/Name`.
    pub signer_name: Option<String>,
    /// Signing date from `/M` (PDF date string, e.g. `D:20260228120000+00'00'`).
    pub sign_date: Option<String>,
    /// Reason for signing from `/Reason`.
    pub reason: Option<String>,
    /// Location of signing from `/Location`.
    pub location: Option<String>,
    /// Contact information from `/ContactInfo`.
    pub contact_info: Option<String>,
    /// `/Filter` name — typically `Adobe.PPKLite` or `ETSI.CAdES.detached`.
    pub filter: Option<String>,
    /// `/SubFilter` name — e.g. `adbe.pkcs7.detached`, `ETSI.CAdES.detached`,
    /// `adbe.x509.rsa_sha1`.
    pub sub_filter: Option<String>,
    /// Byte range of the signed data: `[offset1, len1, offset2, len2]`.
    /// The two ranges together cover everything except the `/Contents` value.
    pub byte_range: Option<[u64; 4]>,
    /// Whether this signature field has been signed (has a `/V` value dict).
    pub is_signed: bool,
}

impl SignatureInfo {
    /// Returns true if the signature uses a detached PKCS#7 envelope
    /// (`adbe.pkcs7.detached` or `ETSI.CAdES.detached`).
    pub fn is_pkcs7_detached(&self) -> bool {
        matches!(
            self.sub_filter.as_deref(),
            Some("adbe.pkcs7.detached") | Some("ETSI.CAdES.detached")
        )
    }

    /// Returns true if the byte range covers the entire document
    /// (offset2 + len2 == file_size).
    ///
    /// Call this with the actual file size. When the range covers the whole
    /// document, the signature was applied to the complete file contents —
    /// a stronger guarantee than a partial signature.
    pub fn covers_entire_document(&self, file_size: u64) -> bool {
        let Some([o1, l1, o2, l2]) = self.byte_range else {
            return false;
        };
        // Covered bytes: [o1..o1+l1] ++ [o2..o2+l2]
        // Entire doc: o1 == 0 && o2+l2 == file_size
        o1 == 0 && (o2 + l2 == file_size)
        // l1 == o2 is guaranteed by PDF spec but we don't enforce it here
        // (malformed PDFs may not comply)
        && (o1 + l1 == o2)
    }
}

/// Result of cryptographically verifying a PDF signature.
///
/// Produced by `pdf.verify_signature(idx)` when the `signatures` feature
/// is enabled.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SignatureVerification {
    /// Whether the mathematical signature is valid (digest matches, sig validates).
    /// Does NOT imply the certificate is trusted — check `cert_chain`.
    pub is_valid: bool,
    /// Common name from the signing certificate's Subject DN.
    pub signer_cn: Option<String>,
    /// Email address from the signing certificate (SubjectAltName or emailAddress).
    pub signer_email: Option<String>,
    /// Whether the signed byte range covers the entire document.
    pub covers_entire_document: bool,
    /// Parsed certificate chain from the CMS SignedData, leaf-first.
    pub cert_chain: Vec<CertInfo>,
    /// Digest algorithm used (e.g. `SHA-256`, `SHA-1`).
    pub digest_algorithm: Option<String>,
    /// Human-readable error if `is_valid` is false.
    pub error: Option<String>,
}

/// Information about a single certificate in the chain.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CertInfo {
    /// Common name from the Subject DN.
    pub subject_cn: Option<String>,
    /// Issuer common name.
    pub issuer_cn: Option<String>,
    /// Not-before validity date (ISO 8601).
    pub not_before: Option<String>,
    /// Not-after validity date (ISO 8601).
    pub not_after: Option<String>,
    /// SHA-256 fingerprint (hex-encoded).
    pub sha256_fingerprint: Option<String>,
    /// Whether this certificate is self-signed (subject == issuer).
    pub is_self_signed: bool,
}

/// Raw signature data extracted from the PDF — used internally by the
/// `signatures` module. Not part of the public API.
///
/// This type carries the cryptographic payload (DER-encoded PKCS#7 and the
/// signed byte offsets) so that `SignatureVerification` can be computed
/// without re-parsing the PDF.
#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct RawSignature {
    /// Metadata (also returned to the user as `SignatureInfo`).
    pub info: SignatureInfo,
    /// Raw DER-encoded CMS/PKCS#7 `SignedData` from the `/Contents` entry.
    /// Empty if the field was unsigned or contents could not be extracted.
    pub pkcs7_der: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_info(byte_range: Option<[u64; 4]>) -> SignatureInfo {
        SignatureInfo {
            signer_name: Some("Test Signer".to_string()),
            sign_date: Some("D:20260228120000+00'00'".to_string()),
            reason: Some("Approval".to_string()),
            location: Some("Seoul".to_string()),
            contact_info: Some("test@example.com".to_string()),
            filter: Some("Adobe.PPKLite".to_string()),
            sub_filter: Some("adbe.pkcs7.detached".to_string()),
            byte_range,
            is_signed: true,
        }
    }

    #[test]
    fn signature_info_covers_entire_document_yes() {
        // File: 1000 bytes. Byte range: [0, 400, 400, 600] — covers all but the hex contents.
        // o1=0, l1=400, o2=400, l2=600 → o1+l1==o2 (400==400) ✓, o2+l2==1000 ✓
        let sig = make_info(Some([0, 400, 400, 600]));
        assert!(sig.covers_entire_document(1000));
    }

    #[test]
    fn signature_info_covers_entire_document_no_gap() {
        // Range doesn't reach end of file
        let sig = make_info(Some([0, 400, 600, 300]));
        assert!(!sig.covers_entire_document(1000));
    }

    #[test]
    fn signature_info_covers_entire_document_no_start_from_zero() {
        let sig = make_info(Some([10, 400, 600, 400]));
        assert!(!sig.covers_entire_document(1000));
    }

    #[test]
    fn signature_info_covers_entire_document_no_byte_range() {
        let sig = make_info(None);
        assert!(!sig.covers_entire_document(1000));
    }

    #[test]
    fn is_pkcs7_detached_adbe() {
        let sig = make_info(None);
        assert!(sig.is_pkcs7_detached());
    }

    #[test]
    fn is_pkcs7_detached_etsi() {
        let mut sig = make_info(None);
        sig.sub_filter = Some("ETSI.CAdES.detached".to_string());
        assert!(sig.is_pkcs7_detached());
    }

    #[test]
    fn is_pkcs7_detached_rsa_sha1_is_not_detached() {
        let mut sig = make_info(None);
        sig.sub_filter = Some("adbe.x509.rsa_sha1".to_string());
        assert!(!sig.is_pkcs7_detached());
    }

    #[test]
    fn signature_info_clone_eq() {
        let sig1 = make_info(Some([0, 100, 200, 300]));
        let sig2 = sig1.clone();
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn signature_info_unsigned() {
        let sig = SignatureInfo {
            signer_name: None,
            sign_date: None,
            reason: None,
            location: None,
            contact_info: None,
            filter: None,
            sub_filter: None,
            byte_range: None,
            is_signed: false,
        };
        assert!(!sig.is_signed);
        assert!(!sig.covers_entire_document(1000));
        assert!(!sig.is_pkcs7_detached());
    }

    #[test]
    fn cert_info_self_signed() {
        let cert = CertInfo {
            subject_cn: Some("ACME Root CA".to_string()),
            issuer_cn: Some("ACME Root CA".to_string()),
            not_before: Some("2020-01-01T00:00:00Z".to_string()),
            not_after: Some("2030-01-01T00:00:00Z".to_string()),
            sha256_fingerprint: Some("aa:bb:cc".to_string()),
            is_self_signed: true,
        };
        assert!(cert.is_self_signed);
        assert_eq!(cert.subject_cn, cert.issuer_cn);
    }

    #[test]
    fn signature_verification_valid() {
        let v = SignatureVerification {
            is_valid: true,
            signer_cn: Some("John Doe".to_string()),
            signer_email: Some("john@example.com".to_string()),
            covers_entire_document: true,
            cert_chain: vec![],
            digest_algorithm: Some("SHA-256".to_string()),
            error: None,
        };
        assert!(v.is_valid);
        assert!(v.error.is_none());
    }

    #[test]
    fn signature_verification_invalid() {
        let v = SignatureVerification {
            is_valid: false,
            signer_cn: None,
            signer_email: None,
            covers_entire_document: false,
            cert_chain: vec![],
            digest_algorithm: None,
            error: Some("digest mismatch".to_string()),
        };
        assert!(!v.is_valid);
        assert!(v.error.is_some());
    }
}
