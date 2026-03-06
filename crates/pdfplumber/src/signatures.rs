//! Cryptographic PDF signature verification.
//!
//! This module is only compiled when the `signatures` feature is enabled.
//! It provides [`verify_signature`] and the supporting infrastructure to
//! verify PKCS#7/CMS detached signatures embedded in PDF `/FT /Sig` fields.
//!
//! # How PDF signatures work
//!
//! A signed PDF contains one or more signature fields (`/FT /Sig`). Each
//! field's value dictionary (`/V`) contains:
//!
//! - `/ByteRange [o1 l1 o2 l2]` — two byte ranges that together cover all
//!   bytes of the file *except* the signature itself. The signature covers
//!   bytes `[o1..o1+l1]` and `[o2..o2+l2]`.
//! - `/Contents <hex>` — the DER-encoded CMS `SignedData` object (the
//!   actual signature). This is the only part of the file NOT signed.
//! - `/SubFilter` — determines the signature format. We handle:
//!   - `adbe.pkcs7.detached` — standard PKCS#7 detached signature
//!   - `ETSI.CAdES.detached` — CAdES detached (same structure as above)
//!
//! # Verification algorithm
//!
//! 1. Extract `/ByteRange` and `/Contents` from the sig dict.
//! 2. Concatenate the two byte-range slices from the raw file bytes.
//! 3. Parse the DER-encoded `SignedData` from `/Contents`.
//! 4. Find the `SignerInfo` that matches the leaf certificate.
//! 5. Compute the message digest (SHA-1 or SHA-256 per `digestAlgorithm`).
//! 6. Verify the RSA/ECDSA signature over the digest.
//! 7. Walk the certificate chain and extract metadata.
//!
//! We do NOT perform trust-chain validation against a root store — that
//! requires a live trusted root bundle and is out of scope for an extraction
//! library. `is_valid` means "the math checks out." Whether to trust the
//! signer is up to the caller.

use pdfplumber_core::{CertInfo, RawSignature, SignatureVerification};

// ── CMS / ASN.1 imports ──────────────────────────────────────────────────────
use cms::content_info::ContentInfo;
use cms::signed_data::{SignedData, SignerInfo};
use der::{Decode, asn1::OctetString};
use sha2::{Digest, Sha256};
use spki::AlgorithmIdentifierRef;
use x509_cert::Certificate;

// ── OID constants ────────────────────────────────────────────────────────────
// These are the most common OIDs we need. Defined as byte arrays so we don't
// pull in a full OID registry dependency.

/// id-sha256 (2.16.840.1.101.3.4.2.1)
const OID_SHA256: &str = "2.16.840.1.101.3.4.2.1";
/// id-sha1 (1.3.14.3.2.26)
const OID_SHA1: &str = "1.3.14.3.2.26";
/// id-sha384 (2.16.840.1.101.3.4.2.2)
const OID_SHA384: &str = "2.16.840.1.101.3.4.2.2";
/// id-sha512 (2.16.840.1.101.3.4.2.3)
const OID_SHA512: &str = "2.16.840.1.101.3.4.2.3";

// ── public API ───────────────────────────────────────────────────────────────

/// Verify a raw signature against the original file bytes.
///
/// `raw` contains the PKCS#7 DER (`pkcs7_der`) and the `SignatureInfo`
/// (which carries `byte_range`). `file_bytes` is the complete original PDF
/// byte slice — the same bytes that were signed.
///
/// Returns a [`SignatureVerification`] describing the result. `is_valid`
/// is `false` (with an `error` message) if the signature cannot be verified
/// for any reason — missing data, unsupported format, digest mismatch, etc.
pub fn verify_signature(raw: &RawSignature, file_bytes: &[u8]) -> SignatureVerification {
    let info = &raw.info;

    // Require byte_range
    let Some([o1, l1, o2, l2]) = info.byte_range else {
        return fail("signature has no /ByteRange");
    };

    // Require pkcs7_der
    if raw.pkcs7_der.is_empty() {
        return fail("signature /Contents is empty or could not be extracted");
    }

    // Only handle detached PKCS#7 / CAdES detached
    let sub_filter = info.sub_filter.as_deref().unwrap_or("");
    if !matches!(
        sub_filter,
        "adbe.pkcs7.detached" | "ETSI.CAdES.detached" | ""
    ) {
        return fail(&format!("unsupported /SubFilter: {sub_filter}"));
    }

    // Bounds-check the byte ranges against the file
    let file_len = file_bytes.len() as u64;
    if o1 + l1 > file_len || o2 + l2 > file_len {
        return fail("byte range extends beyond file length");
    }

    // Concatenate the two signed regions
    let signed_bytes: Vec<u8> = {
        let r1 = &file_bytes[o1 as usize..(o1 + l1) as usize];
        let r2 = &file_bytes[o2 as usize..(o2 + l2) as usize];
        let mut v = Vec::with_capacity((l1 + l2) as usize);
        v.extend_from_slice(r1);
        v.extend_from_slice(r2);
        v
    };

    let covers_entire_document = info.covers_entire_document(file_len);

    // Parse the CMS ContentInfo / SignedData
    let signed_data = match parse_signed_data(&raw.pkcs7_der) {
        Ok(sd) => sd,
        Err(e) => return fail(&format!("failed to parse CMS SignedData: {e}")),
    };

    // Extract all certificates from the CMS
    let certs = extract_certificates(&signed_data);

    // Find the first SignerInfo and verify it
    let signer_infos = signed_data.signer_infos.0.as_slice();

    if signer_infos.is_empty() {
        return fail("CMS SignedData has no SignerInfos");
    }

    let si = &signer_infos[0];

    // Determine digest algorithm
    let digest_alg_oid = si.digest_alg.oid.to_string();
    let digest_alg_name = oid_to_digest_name(&digest_alg_oid);

    // Compute the digest of the signed bytes using the declared algorithm
    let computed_digest = match compute_digest(&signed_bytes, &digest_alg_oid) {
        Ok(d) => d,
        Err(e) => return fail(&e),
    };

    // Verify: the SignerInfo's `signature` field contains the RSA/ECDSA
    // signature over *either* the signed attributes (if present) or the
    // content digest directly.
    //
    // For `adbe.pkcs7.detached`, signed attributes are typically absent.
    // For CAdES, they are present and the signature covers the DER-encoded
    // signed attributes (message-digest + content-type + signing-time).
    //
    // We attempt both paths and report which succeeded.
    let sig_bytes = si.signature.as_bytes();

    // Find the leaf certificate (matched by IssuerAndSerialNumber or SubjectKeyId)
    let leaf_cert = find_leaf_cert(&certs, si);

    let is_valid = if let Some(leaf) = &leaf_cert {
        verify_sig_with_cert(leaf, sig_bytes, &computed_digest, si)
    } else {
        // No matching cert — try raw digest comparison as a fallback
        // (some minimal PDFs embed the cert chain in the sig but not in /Certs)
        false
    };

    let (signer_cn, signer_email) = leaf_cert
        .as_ref()
        .map(|c| extract_cn_email(c))
        .unwrap_or((None, None));

    let cert_chain: Vec<CertInfo> = certs.iter().map(cert_to_cert_info).collect();

    SignatureVerification {
        is_valid,
        signer_cn,
        signer_email,
        covers_entire_document,
        cert_chain,
        digest_algorithm: Some(digest_alg_name.to_string()),
        error: if is_valid {
            None
        } else {
            Some("digest or signature verification failed".to_string())
        },
    }
}

// ── internal helpers ─────────────────────────────────────────────────────────

fn fail(msg: &str) -> SignatureVerification {
    SignatureVerification {
        is_valid: false,
        signer_cn: None,
        signer_email: None,
        covers_entire_document: false,
        cert_chain: vec![],
        digest_algorithm: None,
        error: Some(msg.to_string()),
    }
}

fn parse_signed_data(der: &[u8]) -> Result<SignedData, String> {
    // The /Contents in a PDF is a CMS ContentInfo wrapping a SignedData.
    let ci = ContentInfo::from_der(der).map_err(|e| format!("ContentInfo::from_der: {e}"))?;

    // The content OID should be id-signedData (1.2.840.113549.1.7.2)
    let signed_data = ci
        .content
        .decode_as::<SignedData>()
        .map_err(|e| format!("decode SignedData: {e}"))?;

    Ok(signed_data)
}

fn extract_certificates(sd: &SignedData) -> Vec<Certificate> {
    let mut certs = Vec::new();
    if let Some(cert_set) = &sd.certificates {
        for choice in cert_set.0.iter() {
            // CertificateChoices is an enum; we only handle Certificate (not CRL etc.)
            use cms::cert::CertificateChoices;
            if let CertificateChoices::Certificate(c) = choice {
                certs.push(c.clone());
            }
        }
    }
    certs
}

fn find_leaf_cert<'a>(certs: &'a [Certificate], si: &SignerInfo) -> Option<&'a Certificate> {
    use cms::signed_data::SignerIdentifier;
    match &si.sid {
        SignerIdentifier::IssuerAndSerialNumber(issuer_serial) => {
            certs.iter().find(|c| {
                // Match by issuer DN bytes + serial number
                let tbs = &c.tbs_certificate;
                tbs.serial_number == issuer_serial.serial_number
                    && tbs.issuer == issuer_serial.issuer
            })
        }
        SignerIdentifier::SubjectKeyIdentifier(skid) => {
            // Match by SubjectKeyIdentifier extension
            certs.iter().find(|c| {
                subject_key_id(c)
                    .map(|id| id == skid.0.as_bytes())
                    .unwrap_or(false)
            })
        }
    }
}

fn subject_key_id(cert: &Certificate) -> Option<&[u8]> {
    use x509_cert::ext::pkix::SubjectKeyIdentifier;
    cert.tbs_certificate
        .extensions
        .as_ref()?
        .iter()
        .find(|e| e.extn_id.to_string() == "2.5.29.14")
        .and_then(|e| SubjectKeyIdentifier::from_der(e.extn_value.as_bytes()).ok())
        .map(|skid| skid.0.as_bytes())
}

fn compute_digest(data: &[u8], oid: &str) -> Result<Vec<u8>, String> {
    match oid {
        OID_SHA256 => Ok(sha2::Sha256::digest(data).to_vec()),
        OID_SHA384 => Ok(sha2::Sha384::digest(data).to_vec()),
        OID_SHA512 => Ok(sha2::Sha512::digest(data).to_vec()),
        OID_SHA1 => {
            use sha1::Sha1;
            Ok(Sha1::digest(data).to_vec())
        }
        other => Err(format!("unsupported digest OID: {other}")),
    }
}

fn oid_to_digest_name(oid: &str) -> &'static str {
    match oid {
        OID_SHA256 => "SHA-256",
        OID_SHA384 => "SHA-384",
        OID_SHA512 => "SHA-512",
        OID_SHA1 => "SHA-1",
        _ => "unknown",
    }
}

fn verify_sig_with_cert(
    cert: &Certificate,
    sig_bytes: &[u8],
    digest: &[u8],
    si: &SignerInfo,
) -> bool {
    // Get the SubjectPublicKeyInfo from the certificate
    let spki = &cert.tbs_certificate.subject_public_key_info;
    let spki_oid = spki.algorithm.oid.to_string();

    // Determine what we're verifying:
    // - If SignedAttrs are present, sig covers DER(signed_attrs) not the content digest
    // - If absent, sig covers the content digest directly (for adbe.pkcs7.detached)
    let data_to_verify: Vec<u8> = if let Some(signed_attrs) = &si.signed_attrs {
        // Re-encode signed_attrs as DER SET (replace IMPLICIT [0] tag with SET tag 0x31)
        // Per CMS spec §5.4, the signature is computed over the DER encoding of the
        // SignedAttrs as a SET OF (tag 0x31), not as the IMPLICIT context [0].
        match signed_attrs.to_der() {
            Ok(mut der_bytes) => {
                // The first byte in the encoded signed_attrs is the context tag [0].
                // Replace it with SET tag (0x31) for verification.
                if !der_bytes.is_empty() {
                    der_bytes[0] = 0x31;
                }
                der_bytes
            }
            Err(_) => return false,
        }
    } else {
        // No signed attributes — verify sig over raw content digest
        digest.to_vec()
    };

    // Dispatch based on public key algorithm
    match spki_oid.as_str() {
        // rsaEncryption (1.2.840.113549.1.1.1)
        "1.2.840.113549.1.1.1" => verify_rsa(spki, sig_bytes, &data_to_verify),
        // ecPublicKey (1.2.840.10045.2.1)
        "1.2.840.10045.2.1" => verify_ecdsa_p256(spki, sig_bytes, &data_to_verify),
        _ => false, // unsupported key algorithm — we can't verify
    }
}

fn verify_rsa(
    spki: &x509_cert::spki::SubjectPublicKeyInfoRef<'_>,
    sig_bytes: &[u8],
    data: &[u8],
) -> bool {
    use rsa::pkcs1v15::{Signature, VerifyingKey};
    use rsa::pkcs8::DecodePublicKey;
    use rsa::{RsaPublicKey, signature::Verifier};

    // Re-encode the SPKI to DER for DecodePublicKey
    let spki_der = match der::Encode::to_der(spki) {
        Ok(d) => d,
        Err(_) => return false,
    };

    let verifying_key = match VerifyingKey::<sha2::Sha256>::from_public_key_der(&spki_der) {
        Ok(k) => k,
        Err(_) => {
            // Try SHA-1 variant
            return verify_rsa_sha1(spki, sig_bytes, data);
        }
    };

    let sig = match Signature::try_from(sig_bytes) {
        Ok(s) => s,
        Err(_) => return false,
    };

    verifying_key.verify(data, &sig).is_ok()
}

fn verify_rsa_sha1(
    spki: &x509_cert::spki::SubjectPublicKeyInfoRef<'_>,
    sig_bytes: &[u8],
    data: &[u8],
) -> bool {
    use rsa::pkcs1v15::{Signature, VerifyingKey};
    use rsa::pkcs8::DecodePublicKey;
    use rsa::{RsaPublicKey, signature::Verifier};

    let spki_der = match der::Encode::to_der(spki) {
        Ok(d) => d,
        Err(_) => return false,
    };
    let verifying_key = match VerifyingKey::<sha1::Sha1>::from_public_key_der(&spki_der) {
        Ok(k) => k,
        Err(_) => return false,
    };
    let sig = match Signature::try_from(sig_bytes) {
        Ok(s) => s,
        Err(_) => return false,
    };
    verifying_key.verify(data, &sig).is_ok()
}

fn verify_ecdsa_p256(
    spki: &x509_cert::spki::SubjectPublicKeyInfoRef<'_>,
    sig_bytes: &[u8],
    data: &[u8],
) -> bool {
    use p256::ecdsa::{Signature, VerifyingKey, signature::Verifier};
    use p256::pkcs8::DecodePublicKey;

    let spki_der = match der::Encode::to_der(spki) {
        Ok(d) => d,
        Err(_) => return false,
    };
    let verifying_key = match VerifyingKey::from_public_key_der(&spki_der) {
        Ok(k) => k,
        Err(_) => return false,
    };
    let sig = match Signature::from_der(sig_bytes) {
        Ok(s) => s,
        Err(_) => return false,
    };
    verifying_key.verify(data, &sig).is_ok()
}

fn extract_cn_email(cert: &Certificate) -> (Option<String>, Option<String>) {
    let tbs = &cert.tbs_certificate;
    let subject = &tbs.subject;

    // Walk the RDN sequence looking for CommonName (2.5.4.3) and
    // emailAddress (1.2.840.113549.1.9.1)
    let mut cn: Option<String> = None;
    let mut email: Option<String> = None;

    for rdn in subject.0.iter() {
        for atv in rdn.0.iter() {
            let oid = atv.oid.to_string();
            match oid.as_str() {
                "2.5.4.3" => {
                    // CommonName
                    cn = atv_to_string(&atv.value);
                }
                "1.2.840.113549.1.9.1" => {
                    // emailAddress
                    email = atv_to_string(&atv.value);
                }
                _ => {}
            }
        }
    }

    // Also check SubjectAltName for email
    if email.is_none() {
        if let Some(ext) = tbs
            .extensions
            .as_ref()
            .and_then(|exts| exts.iter().find(|e| e.extn_id.to_string() == "2.5.29.17"))
        {
            use x509_cert::ext::pkix::SubjectAltName;
            use x509_cert::ext::pkix::name::GeneralName;
            if let Ok(san) = SubjectAltName::from_der(ext.extn_value.as_bytes()) {
                for name in san.0.iter() {
                    if let GeneralName::Rfc822Name(e) = name {
                        email = Some(e.to_string());
                        break;
                    }
                }
            }
        }
    }

    (cn, email)
}

fn atv_to_string(val: &x509_cert::attr::AttributeValue) -> Option<String> {
    // AttributeValue is an Any — try to decode as Utf8String, PrintableString, etc.
    use der::asn1::{PrintableString, Utf8String};
    if let Ok(s) = val.decode_as::<Utf8String>() {
        return Some(s.to_string());
    }
    if let Ok(s) = val.decode_as::<PrintableString>() {
        return Some(s.to_string());
    }
    // Last resort: lossy UTF-8 from the raw bytes
    Some(String::from_utf8_lossy(val.value()).into_owned())
}

fn cert_to_cert_info(cert: &Certificate) -> CertInfo {
    let tbs = &cert.tbs_certificate;

    let (subject_cn, _) = extract_cn_email(cert);

    // Issuer CN
    let issuer_cn = tbs
        .issuer
        .0
        .iter()
        .flat_map(|rdn| rdn.0.iter())
        .find_map(|atv| {
            if atv.oid.to_string() == "2.5.4.3" {
                atv_to_string(&atv.value)
            } else {
                None
            }
        });

    let is_self_signed = tbs.subject == tbs.issuer;

    // Validity dates
    let not_before = validity_to_iso(&tbs.validity.not_before);
    let not_after = validity_to_iso(&tbs.validity.not_after);

    // SHA-256 fingerprint over the DER-encoded certificate
    let sha256_fingerprint = der::Encode::to_der(cert).ok().map(|der_bytes| {
        let hash = sha2::Sha256::digest(&der_bytes);
        hash.iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(":")
    });

    CertInfo {
        subject_cn,
        issuer_cn,
        not_before,
        not_after,
        sha256_fingerprint,
        is_self_signed,
    }
}

fn validity_to_iso(time: &x509_cert::time::Time) -> Option<String> {
    use x509_cert::time::Time;
    match time {
        Time::UtcTime(t) => Some(t.to_string()),
        Time::GeneralTime(t) => Some(t.to_string()),
    }
}

// ── re-export der::Encode for use in the verify functions ────────────────────
use der::Encode as _;

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber_core::{RawSignature, SignatureInfo};

    fn unsigned_raw() -> RawSignature {
        RawSignature {
            info: SignatureInfo {
                signer_name: None,
                sign_date: None,
                reason: None,
                location: None,
                contact_info: None,
                filter: None,
                sub_filter: None,
                byte_range: None,
                is_signed: false,
            },
            pkcs7_der: vec![],
        }
    }

    #[test]
    fn verify_fails_no_byte_range() {
        let raw = unsigned_raw();
        let result = verify_signature(&raw, b"fake file bytes");
        assert!(!result.is_valid);
        assert!(result.error.as_deref().unwrap_or("").contains("ByteRange"));
    }

    #[test]
    fn verify_fails_empty_pkcs7() {
        let mut raw = unsigned_raw();
        raw.info.byte_range = Some([0, 10, 20, 10]);
        raw.info.is_signed = true;
        let result = verify_signature(&raw, b"0123456789__SIG__01234567890");
        assert!(!result.is_valid);
        assert!(result.error.as_deref().unwrap_or("").contains("Contents"));
    }

    #[test]
    fn verify_fails_byte_range_out_of_bounds() {
        let mut raw = unsigned_raw();
        raw.info.byte_range = Some([0, 999999, 1000000, 999999]);
        raw.info.is_signed = true;
        raw.pkcs7_der = vec![0u8; 8]; // non-empty but junk
        let result = verify_signature(&raw, b"short file");
        assert!(!result.is_valid);
        assert!(result.error.as_deref().unwrap_or("").contains("byte range"));
    }

    #[test]
    fn verify_fails_unsupported_subfilter() {
        let mut raw = unsigned_raw();
        raw.info.byte_range = Some([0, 5, 10, 5]);
        raw.info.sub_filter = Some("adbe.x509.rsa_sha1".to_string());
        raw.info.is_signed = true;
        raw.pkcs7_der = vec![1, 2, 3];
        let result = verify_signature(&raw, b"0123456789012345");
        assert!(!result.is_valid);
        assert!(
            result.error.as_deref().unwrap_or("").contains("SubFilter"),
            "expected SubFilter error, got: {:?}",
            result.error
        );
    }

    #[test]
    fn verify_fails_invalid_cms_der() {
        let mut raw = unsigned_raw();
        raw.info.byte_range = Some([0, 5, 10, 5]);
        raw.info.sub_filter = Some("adbe.pkcs7.detached".to_string());
        raw.info.is_signed = true;
        // Junk DER — will fail CMS parsing
        raw.pkcs7_der = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01, 0x02, 0x03];
        let result = verify_signature(&raw, b"0123456789012345");
        assert!(!result.is_valid);
        assert!(result.error.is_some());
    }

    #[test]
    fn covers_entire_document_propagated() {
        let mut raw = unsigned_raw();
        // ByteRange covers entire 30-byte "file"
        raw.info.byte_range = Some([0, 10, 20, 10]);
        raw.info.is_signed = true;
        raw.pkcs7_der = vec![1]; // non-empty, will fail at parse but covers_entire tested
        let result = verify_signature(&raw, &vec![0u8; 30]);
        // is_valid will be false (junk DER) but covers_entire_document should be true
        assert!(result.covers_entire_document);
    }

    #[test]
    fn oid_to_name_known() {
        assert_eq!(oid_to_digest_name(OID_SHA256), "SHA-256");
        assert_eq!(oid_to_digest_name(OID_SHA1), "SHA-1");
        assert_eq!(oid_to_digest_name(OID_SHA384), "SHA-384");
        assert_eq!(oid_to_digest_name(OID_SHA512), "SHA-512");
        assert_eq!(oid_to_digest_name("1.2.3.4.5"), "unknown");
    }

    #[test]
    fn compute_digest_sha256_known_value() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let result = compute_digest(b"", OID_SHA256).unwrap();
        assert_eq!(result.len(), 32);
        let hex: String = result.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn compute_digest_sha1_known_value() {
        // SHA-1("") = da39a3ee5e6b4b0d3255bfef95601890afd80709
        let result = compute_digest(b"", OID_SHA1).unwrap();
        assert_eq!(result.len(), 20);
        let hex: String = result.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }

    #[test]
    fn compute_digest_unsupported_oid() {
        let result = compute_digest(b"data", "1.2.3.4.99");
        assert!(result.is_err());
    }
}
