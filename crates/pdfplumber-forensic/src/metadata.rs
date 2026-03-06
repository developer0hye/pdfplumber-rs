//! Extended metadata extraction: richly typed DocInfo summary built from
//! a [`pdfplumber_core::ForensicReport`].

use pdfplumber_core::ForensicReport;

/// Extended PDF metadata derived from a [`ForensicReport`].
///
/// Flattens the report into a convenient struct covering both DocInfo
/// string fields *and* PDF format properties (version, encryption, etc.)
/// in one place.
#[derive(Debug, Clone, Default)]
pub struct ExtendedMetadata {
    // ── DocInfo fields ─────────────────────────────────────────────────────
    /// Document title.
    pub title: Option<String>,
    /// Document author (filled by the application if present).
    pub author: Option<String>,
    /// Subject or description.
    pub subject: Option<String>,
    /// Comma-separated keywords.
    pub keywords: Option<String>,
    /// Application that created the source document (e.g. "Microsoft Word").
    pub creator: Option<String>,
    /// PDF library that wrote this file (e.g. "Adobe PDF Library 21.11").
    pub producer: Option<String>,
    /// Creation date as raw PDF date string (e.g. `D:20240101120000+00'00'`).
    pub creation_date: Option<String>,
    /// Modification date as raw PDF date string.
    pub mod_date: Option<String>,

    // ── Format properties ──────────────────────────────────────────────────
    /// PDF format version (e.g. `"1.7"`, `"2.0"`).
    pub pdf_version: String,
    /// Number of incremental-update cross-reference sections detected.
    /// A value of 1 means the document was never modified after initial save.
    /// Values > 1 indicate post-creation edits.
    pub incremental_update_count: usize,
    /// Whether the document contains any signature fields.
    pub has_signatures: bool,
    /// Number of signature fields detected.
    pub signature_count: usize,
    /// Whether all signature fields are signed (none left blank).
    pub all_signatures_signed: bool,
    /// Total number of pages.
    pub page_count: usize,
    /// Whether creation_date and mod_date are identical.
    ///
    /// This can be perfectly normal (document created and never modified),
    /// but it is also a common pattern when dates are forged wholesale.
    pub dates_identical: bool,
}

impl ExtendedMetadata {
    /// Derive from a fully-built [`ForensicReport`].
    pub fn from_report(report: &ForensicReport) -> Self {
        Self {
            // DocInfo
            title: None,  // ForensicReport does not carry the full DocInfo subset;
            author: None, // consumers who need these should call pdf.metadata() directly.
            subject: None,
            keywords: None,
            creator: report.creator_raw.clone(),
            producer: report.producer_raw.clone(),
            creation_date: report.creation_date.clone(),
            mod_date: report.mod_date.clone(),

            // Format
            pdf_version: report.pdf_version.clone(),
            incremental_update_count: report.incremental_updates.len(),
            has_signatures: !report.signatures.is_empty(),
            signature_count: report.signatures.len(),
            all_signatures_signed: report.all_signatures_signed,
            page_count: report.page_count,
            dates_identical: report.creation_equals_mod_date,
        }
    }

    /// Returns `true` if the metadata appears to be absent or minimal.
    ///
    /// A document with no creator and no producer was likely stripped of
    /// identifying information — a common step in forgery workflows.
    pub fn is_stripped(&self) -> bool {
        self.creator.is_none() && self.producer.is_none()
    }

    /// Render a human-readable metadata card.
    pub fn display(&self) -> String {
        let mut out = String::new();
        let f = |label: &str, v: &Option<String>| -> String {
            format!("  {:18} {}\n", label, v.as_deref().unwrap_or("—"))
        };
        out.push_str("=== Document Metadata ===\n");
        out.push_str(&f("Title:", &self.title));
        out.push_str(&f("Author:", &self.author));
        out.push_str(&f("Subject:", &self.subject));
        out.push_str(&f("Keywords:", &self.keywords));
        out.push_str(&f("Creator:", &self.creator));
        out.push_str(&f("Producer:", &self.producer));
        out.push_str(&f("Created:", &self.creation_date));
        out.push_str(&f("Modified:", &self.mod_date));
        out.push('\n');
        out.push_str("=== Format Properties ===\n");
        out.push_str(&format!("  {:18} {}\n", "PDF Version:", self.pdf_version));
        out.push_str(&format!("  {:18} {} pages\n", "Pages:", self.page_count));
        out.push_str(&format!(
            "  {:18} {}\n",
            "Incr. updates:", self.incremental_update_count
        ));
        out.push_str(&format!(
            "  {:18} {} ({})\n",
            "Signatures:",
            if self.has_signatures { "yes" } else { "no" },
            self.signature_count
        ));
        if self.has_signatures {
            out.push_str(&format!(
                "  {:18} {}\n",
                "All signed:",
                if self.all_signatures_signed {
                    "yes"
                } else {
                    "NO ⚠"
                }
            ));
        }
        if self.dates_identical {
            out.push_str("  ⚠ creation_date == mod_date (possible date forgery)\n");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stub_report() -> ForensicReport {
        use pdfplumber_core::IncrementalUpdate;
        ForensicReport {
            producer_kind: None,
            producer_raw: Some("Test Producer 1.0".to_owned()),
            creator_raw: Some("Microsoft Word".to_owned()),
            pdf_version: "1.7".to_owned(),
            creation_date: Some("D:20240101120000".to_owned()),
            mod_date: Some("D:20240215093000".to_owned()),
            creation_equals_mod_date: false,
            incremental_updates: vec![IncrementalUpdate {
                revision: 1,
                startxref_offset: 0,
                contains_signature_hint: false,
            }],
            has_signature_hint: false,
            signatures: vec![],
            all_signatures_signed: true,
            watermark_findings: vec![],
            page_geometry_anomalies: vec![],
            page_count: 5,
            rotated_page_count: 0,
            metadata_findings: vec![],
            risk_score: 0,
            risk_summary: vec![],
        }
    }

    #[test]
    fn from_report_basic_fields() {
        let r = stub_report();
        let m = ExtendedMetadata::from_report(&r);
        assert_eq!(m.pdf_version, "1.7");
        assert_eq!(m.page_count, 5);
        assert_eq!(m.incremental_update_count, 1);
        assert!(!m.dates_identical);
        assert!(!m.has_signatures);
    }

    #[test]
    fn stripped_when_no_creator_producer() {
        let m = ExtendedMetadata::default();
        assert!(m.is_stripped());
    }

    #[test]
    fn not_stripped_with_producer() {
        let m = ExtendedMetadata {
            producer: Some("Adobe".to_owned()),
            ..Default::default()
        };
        assert!(!m.is_stripped());
    }

    #[test]
    fn dates_identical_flag() {
        let mut r = stub_report();
        r.creation_equals_mod_date = true;
        r.creation_date = Some("D:20240101120000".to_owned());
        r.mod_date = Some("D:20240101120000".to_owned());
        let m = ExtendedMetadata::from_report(&r);
        assert!(m.dates_identical);
        assert!(m.display().contains("date forgery"));
    }

    #[test]
    fn display_contains_key_fields() {
        let mut r = stub_report();
        r.page_count = 12;
        let m = ExtendedMetadata::from_report(&r);
        let d = m.display();
        assert!(d.contains("12 pages"));
        assert!(d.contains("1.7"));
    }
}
