//! High-level forensic inspector — wraps [`Pdf::inspect()`] into a richer
//! structured summary with anomaly scoring and formatted CLI output.

use pdfplumber::Pdf;
use pdfplumber_core::ForensicReport;

use crate::ExtendedMetadata;

/// High-level forensic inspector for PDF documents.
///
/// The primary entry points are:
/// - [`ForensicInspector::inspect`] — when you already have a `Pdf` open
/// - [`ForensicInspector::inspect_bytes`] — opens the PDF from raw bytes
///
/// Both produce a [`ForensicSummary`].
pub struct ForensicInspector;

impl ForensicInspector {
    /// Inspect a `Pdf` document using its raw bytes.
    ///
    /// The `raw_bytes` parameter must be the same bytes used to open the `Pdf`.
    /// They are needed for byte-level incremental-update detection.
    ///
    /// # Example
    /// ```no_run
    /// use pdfplumber::Pdf;
    /// use pdfplumber_forensic::ForensicInspector;
    ///
    /// let bytes = std::fs::read("contract.pdf").unwrap();
    /// let pdf = Pdf::open(&bytes, None).unwrap();
    /// let summary = ForensicInspector::inspect(&pdf, &bytes);
    /// println!("{}", summary.report_text());
    /// ```
    pub fn inspect(pdf: &Pdf, raw_bytes: &[u8]) -> ForensicSummary {
        let core_report = pdf.inspect(raw_bytes);
        let metadata = ExtendedMetadata::from_report(&core_report);
        let anomaly_score = Self::compute_score(&core_report);
        ForensicSummary {
            core_report,
            metadata,
            anomaly_score,
        }
    }

    /// Open a PDF from raw bytes and run forensic inspection in one step.
    ///
    /// # Errors
    /// Returns the `pdfplumber` parse error if the document cannot be opened.
    ///
    /// # Example
    /// ```no_run
    /// use pdfplumber_forensic::ForensicInspector;
    ///
    /// let bytes = std::fs::read("contract.pdf").unwrap();
    /// let summary = ForensicInspector::inspect_bytes(&bytes).unwrap();
    /// println!("{}", summary.summary());
    /// ```
    pub fn inspect_bytes(raw_bytes: &[u8]) -> Result<ForensicSummary, pdfplumber::PdfError> {
        let pdf = Pdf::open(raw_bytes, None)?;
        Ok(Self::inspect(&pdf, raw_bytes))
    }

    // ------------------------------------------------------------------
    // Scoring
    // ------------------------------------------------------------------

    /// Compute a heuristic anomaly score 0–100.
    ///
    /// Higher = more suspicious. The individual weights are empirically
    /// calibrated to balance false-positive rate against sensitivity.
    fn compute_score(report: &ForensicReport) -> u8 {
        let mut score: u32 = 0;

        // Incremental updates without a corresponding signature are suspicious
        let unsignable_updates = report
            .incremental_updates
            .len()
            .saturating_sub(report.signatures.len())
            .saturating_sub(1); // first revision is always the original creation
        score += (unsignable_updates as u32).min(3) * 10;

        // Unsigned signature fields: the document claims signatures but not all signed
        if !report.signatures.is_empty() && !report.all_signatures_signed {
            score += 15;
        }

        // Watermark-style hidden content
        score += (report.watermark_findings.len() as u32).min(4) * 8;

        // Metadata findings (scrubbed fields, date inconsistencies, etc.)
        score += (report.metadata_findings.len() as u32).min(5) * 5;

        // Identical creation/modification dates (suspicious in combination with
        // other signals, but not alarming by itself)
        if report.creation_equals_mod_date && report.incremental_updates.len() > 1 {
            score += 10;
        }

        // Page geometry anomalies
        score += (report.page_geometry_anomalies.len() as u32).min(3) * 5;

        score.min(100) as u8
    }
}

/// A complete forensic inspection result.
///
/// Wraps the raw [`ForensicReport`] from `pdfplumber-core` and adds:
/// - A normalised anomaly score (0–100)
/// - Flattened [`ExtendedMetadata`]
/// - Rich formatted text output
pub struct ForensicSummary {
    /// The underlying core forensic report.
    pub core_report: ForensicReport,
    /// Flattened metadata view.
    pub metadata: ExtendedMetadata,
    /// Heuristic anomaly score (0 = clean, 100 = very suspicious).
    pub anomaly_score: u8,
}

impl ForensicSummary {
    /// Returns the anomaly score (0–100).
    pub fn anomaly_score(&self) -> u8 {
        self.anomaly_score
    }

    /// Returns `true` if the document appears completely clean.
    ///
    /// A clean document has score 0 and `core_report.is_clean() == true`.
    pub fn is_clean(&self) -> bool {
        self.anomaly_score == 0 && self.core_report.is_clean()
    }

    /// Returns a short one-line verdict string suitable for CLI status output.
    pub fn verdict(&self) -> &'static str {
        match self.anomaly_score {
            0 => "CLEAN",
            1..=24 => "LOW RISK",
            25..=49 => "MODERATE RISK",
            50..=74 => "HIGH RISK",
            _ => "CRITICAL",
        }
    }

    /// Returns the full formatted text report (delegates to `ForensicReport::format_text()`
    /// plus the extended metadata card and score banner).
    pub fn report_text(&self) -> String {
        let mut out = String::new();

        // Score banner
        out.push_str(&format!(
            "╔══════════════════════════════════════════════════════════╗\n\
             ║  FORENSIC INSPECTION REPORT                               ║\n\
             ║  Anomaly Score: {:3}/100  │  Verdict: {:12}        ║\n\
             ╚══════════════════════════════════════════════════════════╝\n\n",
            self.anomaly_score,
            self.verdict()
        ));

        // Core report (producer, watermarks, incremental updates, etc.)
        out.push_str(&self.core_report.format_text());
        out.push('\n');

        // Extended metadata card
        out.push_str(&self.metadata.display());

        out
    }

    /// Short summary (3–5 lines) suitable for a single-document audit log entry.
    pub fn summary(&self) -> String {
        format!(
            "Forensic score: {}/100 [{}] | pages: {} | incremental updates: {} | \
             watermarks: {} | metadata findings: {} | signatures: {}",
            self.anomaly_score,
            self.verdict(),
            self.metadata.page_count,
            self.metadata.incremental_update_count,
            self.core_report.watermark_findings.len(),
            self.core_report.metadata_findings.len(),
            self.metadata.signature_count,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber_core::{IncrementalUpdate, WatermarkFinding, WatermarkKind};

    fn clean_report() -> ForensicReport {
        ForensicReport {
            producer_kind: None,
            producer_raw: Some("Acrobat 21".to_owned()),
            creator_raw: Some("Word".to_owned()),
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
            page_count: 3,
            rotated_page_count: 0,
            metadata_findings: vec![],
            risk_score: 0,
            risk_summary: vec![],
        }
    }

    #[test]
    fn clean_document_scores_zero() {
        let r = clean_report();
        let score = ForensicInspector::compute_score(&r);
        assert_eq!(score, 0);
    }

    #[test]
    fn watermarks_increase_score() {
        let mut r = clean_report();
        r.watermark_findings.push(WatermarkFinding {
            kind: WatermarkKind::LowOpacityText,
            page_index: 0,
            text_preview: Some("CONFIDENTIAL".to_owned()),
        });
        let score = ForensicInspector::compute_score(&r);
        assert!(score > 0);
    }

    #[test]
    fn extra_incremental_updates_increase_score() {
        let mut r = clean_report();
        r.incremental_updates.push(IncrementalUpdate {
            revision: 2,
            startxref_offset: 9999,
            contains_signature_hint: false,
        });
        r.incremental_updates.push(IncrementalUpdate {
            revision: 3,
            startxref_offset: 19999,
            contains_signature_hint: false,
        });
        let score = ForensicInspector::compute_score(&r);
        assert!(score >= 10, "score was {score}");
    }

    #[test]
    fn verdict_ranges() {
        let s = |n: u8| ForensicSummary {
            core_report: clean_report(),
            metadata: ExtendedMetadata::default(),
            anomaly_score: n,
        };
        assert_eq!(s(0).verdict(), "CLEAN");
        assert_eq!(s(10).verdict(), "LOW RISK");
        assert_eq!(s(30).verdict(), "MODERATE RISK");
        assert_eq!(s(60).verdict(), "HIGH RISK");
        assert_eq!(s(80).verdict(), "CRITICAL");
    }

    #[test]
    fn summary_contains_key_metrics() {
        let s = ForensicSummary {
            core_report: clean_report(),
            metadata: ExtendedMetadata {
                page_count: 7,
                ..Default::default()
            },
            anomaly_score: 0,
        };
        let txt = s.summary();
        assert!(txt.contains("7"), "page count missing: {txt}");
        assert!(txt.contains("CLEAN"), "verdict missing: {txt}");
    }

    #[test]
    fn is_clean_only_when_score_zero_and_core_clean() {
        let clean = ForensicSummary {
            core_report: clean_report(),
            metadata: ExtendedMetadata::default(),
            anomaly_score: 0,
        };
        assert!(clean.is_clean());

        let not_clean = ForensicSummary {
            core_report: clean_report(),
            metadata: ExtendedMetadata::default(),
            anomaly_score: 5,
        };
        assert!(!not_clean.is_clean());
    }
}
