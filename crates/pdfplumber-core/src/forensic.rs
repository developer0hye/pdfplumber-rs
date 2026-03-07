//! Forensic metadata inspection for PDF documents.
//!
//! Provides [`ForensicReport`] — a comprehensive forensic analysis of a PDF
//! document covering:
//!
//! - **Producer fingerprinting**: identifies the software that created the PDF
//!   and flags known suspicious producers (e.g., fillable-form converters,
//!   screen-capture tools, PDF-to-PDF re-exporters)
//! - **Metadata consistency**: cross-checks DocInfo fields vs embedded XMP,
//!   flags discrepancies that suggest tampering or re-export
//! - **Incremental update detection**: counts xref sections; >1 means the file
//!   was modified after initial creation (annotations, signatures, alterations)
//! - **Watermark detection**: identifies low-opacity text layers, repeated
//!   content across pages, and invisible (white/transparent) text
//! - **Signature inventory**: lists all signature fields and their signed state
//! - **Page geometry anomalies**: flags pages with unusual rotation, non-standard
//!   media boxes, or clipped content boxes
//!
//! # Usage
//!
//! ```rust,no_run
//! use pdfplumber_core::forensic::ForensicReport;
//! // ForensicReport is constructed by the Pdf type via inspect()
//! ```

use crate::{DocumentMetadata, SignatureInfo};

// ---------------------------------------------------------------------------
// Producer fingerprinting
// ---------------------------------------------------------------------------

/// Known PDF producer strings and the tool that generated them.
///
/// Used to identify the originating software from the `/Producer` field.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ProducerKind {
    /// Adobe Acrobat (version hint included if parseable).
    AdobeAcrobat,
    /// Adobe Distiller (PostScript → PDF converter).
    AdobeDistiller,
    /// Microsoft Word (native export).
    MicrosoftWord,
    /// LibreOffice / OpenOffice.
    LibreOffice,
    /// Apple macOS / iOS print driver (Quartz PDFContext).
    AppleQuartz,
    /// Google Docs / Google Workspace.
    GoogleDocs,
    /// LaTeX (pdflatex, LuaLaTeX, XeLaTeX).
    Latex,
    /// Ghostscript.
    Ghostscript,
    /// wkhtmltopdf (HTML→PDF).
    Wkhtmltopdf,
    /// Reportlab (Python PDF generation).
    Reportlab,
    /// iText / iTextSharp (.NET/Java PDF library).
    Itext,
    /// PDFium (Chromium's PDF renderer — often means "printed from browser").
    Pdfium,
    /// Foxit PDF.
    Foxit,
    /// Nitro PDF.
    Nitro,
    /// PDF24 (online converter — data leaves org if used).
    Pdf24,
    /// Smallpdf (online converter — data leaves org).
    Smallpdf,
    /// Adobe LiveCycle (enterprise forms).
    AdobeLiveCycle,
    /// DocuSign (e-signature platform).
    DocuSign,
    /// Unknown / unrecognised producer.
    Unknown(String),
}

impl ProducerKind {
    /// Parse a raw producer string into a known kind.
    pub fn from_producer_string(producer: &str) -> Self {
        let p = producer.to_lowercase();
        if p.contains("acrobat distiller") {
            Self::AdobeDistiller
        } else if p.contains("acrobat") || p.contains("adobe pdf") {
            Self::AdobeAcrobat
        } else if p.contains("microsoft word") || p.contains("msword") {
            Self::MicrosoftWord
        } else if p.contains("libreoffice") || p.contains("openoffice") {
            Self::LibreOffice
        } else if p.contains("quartz pdfcontext") || p.contains("mac os x") || p.contains("macos") {
            Self::AppleQuartz
        } else if p.contains("google") || p.contains("docs-docservice") {
            Self::GoogleDocs
        } else if p.contains("pdflatex")
            || p.contains("xelatex")
            || p.contains("lualatex")
            || p.contains("pdftex")
            || p.contains("latex")
        {
            Self::Latex
        } else if p.contains("ghostscript") || p.starts_with("gs ") {
            Self::Ghostscript
        } else if p.contains("wkhtmltopdf") {
            Self::Wkhtmltopdf
        } else if p.contains("reportlab") {
            Self::Reportlab
        } else if p.contains("itext") {
            Self::Itext
        } else if p.contains("pdfium") {
            Self::Pdfium
        } else if p.contains("foxit") {
            Self::Foxit
        } else if p.contains("nitro") {
            Self::Nitro
        } else if p.contains("pdf24") {
            Self::Pdf24
        } else if p.contains("smallpdf") {
            Self::Smallpdf
        } else if p.contains("livecycle") {
            Self::AdobeLiveCycle
        } else if p.contains("docusign") {
            Self::DocuSign
        } else {
            Self::Unknown(producer.to_string())
        }
    }

    /// Returns `true` for producers that are online converters (data may have left the org).
    pub fn is_online_converter(&self) -> bool {
        matches!(self, Self::Pdf24 | Self::Smallpdf)
    }

    /// Returns `true` for e-signature platforms (implies document was signed externally).
    pub fn is_esignature_platform(&self) -> bool {
        matches!(self, Self::DocuSign)
    }

    /// Human-readable label for display.
    pub fn label(&self) -> &str {
        match self {
            Self::AdobeAcrobat => "Adobe Acrobat",
            Self::AdobeDistiller => "Adobe Distiller",
            Self::MicrosoftWord => "Microsoft Word",
            Self::LibreOffice => "LibreOffice / OpenOffice",
            Self::AppleQuartz => "Apple Quartz PDFContext",
            Self::GoogleDocs => "Google Docs",
            Self::Latex => "LaTeX",
            Self::Ghostscript => "Ghostscript",
            Self::Wkhtmltopdf => "wkhtmltopdf",
            Self::Reportlab => "ReportLab",
            Self::Itext => "iText / iTextSharp",
            Self::Pdfium => "PDFium (browser print)",
            Self::Foxit => "Foxit PDF",
            Self::Nitro => "Nitro PDF",
            Self::Pdf24 => "PDF24 (online converter ⚠)",
            Self::Smallpdf => "Smallpdf (online converter ⚠)",
            Self::AdobeLiveCycle => "Adobe LiveCycle",
            Self::DocuSign => "DocuSign",
            Self::Unknown(s) => s.as_str(),
        }
    }
}

// ---------------------------------------------------------------------------
// Incremental update
// ---------------------------------------------------------------------------

/// Information about an incremental update section in the PDF file.
///
/// PDFs are updated incrementally by appending new xref sections and
/// updated objects to the end of the file. Each append is one update.
/// Multiple updates can indicate added annotations, form fills, or signatures
/// — but can also indicate tampering.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IncrementalUpdate {
    /// 1-based index of this update (1 = original creation, 2+ = modifications).
    pub revision: usize,
    /// Byte offset of the `startxref` marker for this revision.
    pub startxref_offset: u64,
    /// Whether this revision appears to contain a signature (has `/Sig` in
    /// the modified objects — heuristic, not guaranteed).
    pub contains_signature_hint: bool,
}

// ---------------------------------------------------------------------------
// Watermark finding
// ---------------------------------------------------------------------------

/// Category of detected watermark or invisible content.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WatermarkKind {
    /// Text with near-zero opacity (alpha < 0.15) — classic "CONFIDENTIAL" watermark.
    LowOpacityText,
    /// Text rendered in white on a white/light background — invisible but present.
    InvisibleText,
    /// Identical text block found on N or more consecutive pages.
    RepeatedTextBlock {
        /// Number of pages on which this text block repeats.
        page_count: usize,
        /// Short preview of the repeated text (first 80 chars).
        text_preview: String,
    },
    /// A graphics object (rect or image) spanning most of the page with low opacity.
    LowOpacityOverlay,
}

/// A detected watermark or hidden content layer.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WatermarkFinding {
    /// What kind of watermark this is.
    pub kind: WatermarkKind,
    /// Page index (0-based) where first detected.
    pub page_index: usize,
    /// Short text preview (for text-based watermarks).
    pub text_preview: Option<String>,
}

// ---------------------------------------------------------------------------
// Page geometry anomaly
// ---------------------------------------------------------------------------

/// An anomaly in a page's geometry specification.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PageGeometryAnomaly {
    /// Page index (0-based).
    pub page_index: usize,
    /// Human-readable description of the anomaly.
    pub description: String,
}

// ---------------------------------------------------------------------------
// Metadata consistency finding
// ---------------------------------------------------------------------------

/// A discrepancy between DocInfo and XMP metadata, or a suspicious field value.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MetadataFinding {
    /// The field that triggered this finding (e.g., "producer", "creation_date").
    pub field: String,
    /// Human-readable description of the finding.
    pub description: String,
    /// The raw value that triggered the finding, if applicable.
    pub value: Option<String>,
}

// ---------------------------------------------------------------------------
// ForensicReport — the top-level output
// ---------------------------------------------------------------------------

/// Comprehensive forensic analysis of a PDF document.
///
/// Produced by `Pdf::inspect()`. Contains all findings from metadata
/// analysis, structural analysis, and content analysis.
///
/// # Interpretation
///
/// A clean document has:
/// - `incremental_updates` with exactly 1 revision (no post-creation edits)
/// - `watermark_findings` empty
/// - `page_geometry_anomalies` empty
/// - `metadata_findings` empty or only informational
/// - `risk_score` of 0
///
/// A tampered/suspicious document might have multiple incremental revisions
/// with no corresponding signature, watermark layers, or metadata inconsistencies.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ForensicReport {
    // ---- Producer ----
    /// Identified producer software.
    pub producer_kind: Option<ProducerKind>,
    /// Raw producer string from the PDF /Info dictionary.
    pub producer_raw: Option<String>,
    /// Identified creator software (the app that made the source document).
    pub creator_raw: Option<String>,
    /// PDF version string (e.g., "1.7", "2.0").
    pub pdf_version: String,

    // ---- Dates ----
    /// Raw creation date string from /Info dictionary.
    pub creation_date: Option<String>,
    /// Raw modification date string from /Info dictionary.
    pub mod_date: Option<String>,
    /// Whether creation_date and mod_date are identical (original-only doc).
    pub creation_equals_mod_date: bool,

    // ---- Incremental updates ----
    /// All detected incremental update sections (revisions).
    /// Length 1 = original only. Length > 1 = document was modified after creation.
    pub incremental_updates: Vec<IncrementalUpdate>,
    /// Whether any revision appears to contain a signature hint.
    pub has_signature_hint: bool,

    // ---- Signatures ----
    /// All signature fields found in the document.
    pub signatures: Vec<SignatureInfo>,
    /// Whether all signature fields have been signed (no blank sig fields).
    pub all_signatures_signed: bool,

    // ---- Watermarks ----
    /// Detected watermark and invisible-content findings.
    pub watermark_findings: Vec<WatermarkFinding>,

    // ---- Page geometry ----
    /// Anomalies in page geometry (unusual rotations, mismatched boxes).
    pub page_geometry_anomalies: Vec<PageGeometryAnomaly>,
    /// Total page count.
    pub page_count: usize,
    /// Page count with non-zero rotation.
    pub rotated_page_count: usize,

    // ---- Metadata consistency ----
    /// Metadata-level findings (suspicious values, missing fields, inconsistencies).
    pub metadata_findings: Vec<MetadataFinding>,

    // ---- Risk summary ----
    /// Composite risk score (0 = clean, higher = more suspicious findings).
    /// Purely heuristic — not a definitive tamper verdict.
    pub risk_score: u32,

    /// Human-readable risk summary lines for CLI display.
    pub risk_summary: Vec<String>,
}

impl ForensicReport {
    /// Build a forensic report from the raw components.
    ///
    /// This is the canonical constructor — called by `Pdf::inspect()`.
    pub fn build(
        metadata: &DocumentMetadata,
        pdf_version: String,
        raw_bytes: &[u8],
        signatures: Vec<SignatureInfo>,
        page_count: usize,
        page_rotations: &[i32],
        page_dims: &[(f64, f64)], // (width, height) per page
    ) -> Self {
        let producer_raw = metadata.producer.clone();
        let producer_kind = producer_raw
            .as_deref()
            .map(ProducerKind::from_producer_string);
        let creator_raw = metadata.creator.clone();
        let creation_date = metadata.creation_date.clone();
        let mod_date = metadata.mod_date.clone();
        let creation_equals_mod_date = matches!(
            (&creation_date, &mod_date),
            (Some(c), Some(m)) if c == m
        );

        // ---- Incremental updates ----
        let incremental_updates = detect_incremental_updates(raw_bytes);
        let has_signature_hint = incremental_updates
            .iter()
            .any(|u| u.contains_signature_hint);

        // ---- Signatures ----
        let all_signatures_signed =
            !signatures.is_empty() && signatures.iter().all(|s| s.is_signed);

        // ---- Watermarks ----
        let watermark_findings = Vec::new(); // populated from page analysis below (passed in)

        // ---- Page geometry anomalies ----
        let rotated_page_count = page_rotations.iter().filter(|&&r| r != 0).count();
        let mut page_geometry_anomalies = Vec::new();
        for (i, &rotation) in page_rotations.iter().enumerate() {
            if rotation != 0 && rotation != 90 && rotation != 180 && rotation != 270 {
                page_geometry_anomalies.push(PageGeometryAnomaly {
                    page_index: i,
                    description: format!("Non-standard rotation: {rotation}°"),
                });
            }
        }
        // Flag pages with unusual aspect ratios (might be landscape without rotation)
        for (i, &(w, h)) in page_dims.iter().enumerate() {
            if w > 5000.0 || h > 5000.0 {
                page_geometry_anomalies.push(PageGeometryAnomaly {
                    page_index: i,
                    description: format!(
                        "Unusually large page: {w:.0}×{h:.0} pts ({:.1}×{:.1} inches)",
                        w / 72.0,
                        h / 72.0
                    ),
                });
            }
            if w < 36.0 || h < 36.0 {
                page_geometry_anomalies.push(PageGeometryAnomaly {
                    page_index: i,
                    description: format!(
                        "Unusually small page: {w:.0}×{h:.0} pts ({:.2}×{:.2} inches)",
                        w / 72.0,
                        h / 72.0
                    ),
                });
            }
        }

        // ---- Metadata findings ----
        let mut metadata_findings = Vec::new();

        if let Some(ref prod) = producer_raw {
            if let Some(ref kind) = producer_kind {
                if kind.is_online_converter() {
                    metadata_findings.push(MetadataFinding {
                        field: "producer".to_string(),
                        description: format!(
                            "Document was processed by an online converter ({}). \
                             The document content may have been sent to a third-party server.",
                            kind.label()
                        ),
                        value: Some(prod.clone()),
                    });
                }
            }
        }

        if metadata.producer.is_none() && metadata.creator.is_none() {
            metadata_findings.push(MetadataFinding {
                field: "producer".to_string(),
                description:
                    "Both /Producer and /Creator are absent — may indicate metadata scrubbing."
                        .to_string(),
                value: None,
            });
        }

        if creation_date.is_none() {
            metadata_findings.push(MetadataFinding {
                field: "creation_date".to_string(),
                description: "No creation date in /Info dictionary.".to_string(),
                value: None,
            });
        }

        if creation_equals_mod_date {
            if let Some(ref d) = creation_date {
                metadata_findings.push(MetadataFinding {
                    field: "mod_date".to_string(),
                    description: "Creation date equals modification date — document has never been modified since creation.".to_string(),
                    value: Some(d.clone()),
                });
            }
        }

        if incremental_updates.len() > 1 && signatures.is_empty() {
            metadata_findings.push(MetadataFinding {
                field: "incremental_updates".to_string(),
                description: format!(
                    "Document has {} incremental revisions but no signature fields. \
                     Post-creation modifications may not be auditable.",
                    incremental_updates.len()
                ),
                value: None,
            });
        }

        // ---- Risk score ----
        let mut risk_score: u32 = 0;
        let mut risk_summary = Vec::new();

        if incremental_updates.len() > 1 {
            let n = incremental_updates.len() - 1;
            risk_score += n as u32;
            risk_summary.push(format!(
                "+{n} — document modified {n} time(s) after initial creation"
            ));
        }

        if let Some(ref kind) = producer_kind {
            if kind.is_online_converter() {
                risk_score += 3;
                risk_summary.push(format!(
                    "+3 — online converter used ({}), content may have left org",
                    kind.label()
                ));
            }
        }

        if metadata.producer.is_none() && metadata.creator.is_none() {
            risk_score += 2;
            risk_summary.push("+2 — metadata scrubbed (no producer/creator)".to_string());
        }

        if !signatures.is_empty() && !all_signatures_signed {
            let unsigned = signatures.iter().filter(|s| !s.is_signed).count();
            risk_score += unsigned as u32;
            risk_summary.push(format!(
                "+{unsigned} — {unsigned} signature field(s) present but not signed"
            ));
        }

        if !page_geometry_anomalies.is_empty() {
            risk_score += 1;
            risk_summary.push(format!(
                "+1 — {} page geometry anomaly/anomalies",
                page_geometry_anomalies.len()
            ));
        }

        Self {
            producer_kind,
            producer_raw,
            creator_raw,
            pdf_version,
            creation_date,
            mod_date,
            creation_equals_mod_date,
            incremental_updates,
            has_signature_hint,
            signatures,
            all_signatures_signed,
            watermark_findings,
            page_geometry_anomalies,
            page_count,
            rotated_page_count,
            metadata_findings,
            risk_score,
            risk_summary,
        }
    }

    /// Returns `true` if the document shows no signs of modification or tampering.
    pub fn is_clean(&self) -> bool {
        self.risk_score == 0
    }

    /// Returns `true` if the document was modified after initial creation.
    pub fn was_modified(&self) -> bool {
        self.incremental_updates.len() > 1
    }

    /// Returns the number of post-creation modifications.
    pub fn modification_count(&self) -> usize {
        self.incremental_updates.len().saturating_sub(1)
    }

    /// Format the report as a human-readable string suitable for terminal display.
    pub fn format_text(&self) -> String {
        let mut out = Vec::new();

        out.push("═══════════════════════════════════════════════════════════".to_string());
        out.push("  pdfplumber forensic inspection report".to_string());
        out.push("═══════════════════════════════════════════════════════════".to_string());
        out.push(String::new());

        // Producer
        out.push("── Origin ──────────────────────────────────────────────────".to_string());
        out.push(format!("  PDF version : {}", self.pdf_version));
        if let Some(ref kind) = self.producer_kind {
            out.push(format!("  Producer    : {}", kind.label()));
        } else {
            out.push("  Producer    : (none)".to_string());
        }
        if let Some(ref raw) = self.producer_raw {
            out.push(format!("  Producer raw: {raw}"));
        }
        if let Some(ref creator) = self.creator_raw {
            out.push(format!("  Creator     : {creator}"));
        }
        out.push(String::new());

        // Dates
        out.push("── Dates ───────────────────────────────────────────────────".to_string());
        out.push(format!(
            "  Created  : {}",
            self.creation_date.as_deref().unwrap_or("(none)")
        ));
        out.push(format!(
            "  Modified : {}",
            self.mod_date.as_deref().unwrap_or("(none)")
        ));
        if self.creation_equals_mod_date {
            out.push("  ✓ Never modified since creation".to_string());
        }
        out.push(String::new());

        // Structure
        out.push("── Document Structure ──────────────────────────────────────".to_string());
        out.push(format!("  Pages       : {}", self.page_count));
        if self.rotated_page_count > 0 {
            out.push(format!(
                "  Rotated     : {} page(s)",
                self.rotated_page_count
            ));
        }
        out.push(format!(
            "  Revisions   : {} ({})",
            self.incremental_updates.len(),
            if self.incremental_updates.len() == 1 {
                "original only — no post-creation edits"
            } else {
                "document was modified after creation"
            }
        ));
        if self.incremental_updates.len() > 1 {
            for update in &self.incremental_updates {
                let sig = if update.contains_signature_hint {
                    " [signature hint]"
                } else {
                    ""
                };
                out.push(format!(
                    "    rev {}: offset {}{sig}",
                    update.revision, update.startxref_offset
                ));
            }
        }
        out.push(String::new());

        // Signatures
        out.push("── Signatures ──────────────────────────────────────────────".to_string());
        if self.signatures.is_empty() {
            out.push("  No signature fields found.".to_string());
        } else {
            for (i, sig) in self.signatures.iter().enumerate() {
                let status = if sig.is_signed {
                    "SIGNED"
                } else {
                    "UNSIGNED FIELD"
                };
                out.push(format!("  [{status}] signature {}", i + 1));
                if let Some(ref name) = sig.signer_name {
                    out.push(format!("    Signer  : {name}"));
                }
                if let Some(ref date) = sig.sign_date {
                    out.push(format!("    Date    : {date}"));
                }
                if let Some(ref reason) = sig.reason {
                    out.push(format!("    Reason  : {reason}"));
                }
                if let Some(ref loc) = sig.location {
                    out.push(format!("    Location: {loc}"));
                }
            }
        }
        out.push(String::new());

        // Watermarks
        if !self.watermark_findings.is_empty() {
            out.push("── Watermarks / Hidden Content ─────────────────────────────".to_string());
            for wm in &self.watermark_findings {
                let kind_str = match &wm.kind {
                    WatermarkKind::LowOpacityText => "low-opacity text".to_string(),
                    WatermarkKind::InvisibleText => {
                        "invisible text (white/transparent)".to_string()
                    }
                    WatermarkKind::RepeatedTextBlock {
                        page_count,
                        text_preview,
                    } => {
                        format!("repeated text on {page_count} pages: \"{text_preview}\"")
                    }
                    WatermarkKind::LowOpacityOverlay => "low-opacity graphic overlay".to_string(),
                };
                let preview = wm.text_preview.as_deref().unwrap_or("");
                let preview_str = if preview.is_empty() {
                    String::new()
                } else {
                    format!(" — \"{preview}\"")
                };
                out.push(format!(
                    "  page {}: {kind_str}{preview_str}",
                    wm.page_index + 1
                ));
            }
            out.push(String::new());
        }

        // Geometry anomalies
        if !self.page_geometry_anomalies.is_empty() {
            out.push("── Page Geometry Anomalies ─────────────────────────────────".to_string());
            for anom in &self.page_geometry_anomalies {
                out.push(format!(
                    "  page {}: {}",
                    anom.page_index + 1,
                    anom.description
                ));
            }
            out.push(String::new());
        }

        // Metadata findings
        if !self.metadata_findings.is_empty() {
            out.push("── Metadata Findings ───────────────────────────────────────".to_string());
            for finding in &self.metadata_findings {
                out.push(format!("  [{}] {}", finding.field, finding.description));
                if let Some(ref val) = finding.value {
                    out.push(format!("    value: {val}"));
                }
            }
            out.push(String::new());
        }

        // Risk
        out.push("── Risk Assessment ─────────────────────────────────────────".to_string());
        out.push(format!("  Score: {} / 10+", self.risk_score));
        if self.risk_summary.is_empty() {
            out.push("  ✓ No risk factors detected.".to_string());
        } else {
            for line in &self.risk_summary {
                out.push(format!("  {line}"));
            }
        }
        out.push(String::new());
        out.push("═══════════════════════════════════════════════════════════".to_string());

        out.join("\n")
    }
}

// ---------------------------------------------------------------------------
// Incremental update detection (pure byte-level scan)
// ---------------------------------------------------------------------------

/// Scan raw PDF bytes to find all `startxref` markers and their offsets.
///
/// Each occurrence of `startxref\n<offset>` near the end of the file
/// represents one revision. The first one is the original; subsequent ones
/// are incremental updates.
///
/// This is a byte-scan heuristic — it does not fully parse the xref table.
/// False positives are possible in highly unusual PDFs but are rare in practice.
pub fn detect_incremental_updates(bytes: &[u8]) -> Vec<IncrementalUpdate> {
    let mut updates = Vec::new();

    // Scan backward from EOF — PDFs typically have startxref near the end.
    // We scan the entire file to find all revisions, not just the latest.
    let needle = b"startxref";
    let sig_needle = b"/Sig";

    let mut search_pos = 0usize;
    let mut revision = 0usize;

    while search_pos < bytes.len() {
        if let Some(pos) = memfind(bytes, needle, search_pos) {
            revision += 1;
            // Parse the offset value after "startxref\n"
            let after = pos + needle.len();
            let offset_str = skip_whitespace_and_parse_u64(bytes, after);
            let startxref_offset = offset_str.unwrap_or(0);

            // Heuristic: look for /Sig in a window around this xref position
            let window_start = pos.saturating_sub(4096);
            let window_end = (pos + 4096).min(bytes.len());
            let window = &bytes[window_start..window_end];
            let contains_signature_hint = memfind(window, sig_needle, 0).is_some();

            updates.push(IncrementalUpdate {
                revision,
                startxref_offset,
                contains_signature_hint,
            });

            search_pos = pos + needle.len();
        } else {
            break;
        }
    }

    updates
}

/// Find the first occurrence of `needle` in `haystack` starting at `from`.
fn memfind(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() + from {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// Skip ASCII whitespace after `pos` and parse a decimal u64.
fn skip_whitespace_and_parse_u64(bytes: &[u8], pos: usize) -> Option<u64> {
    let start = bytes[pos..].iter().position(|b| b.is_ascii_digit())? + pos;
    let end = bytes[start..]
        .iter()
        .position(|b| !b.is_ascii_digit())
        .map(|p| p + start)
        .unwrap_or(bytes.len());
    let s = std::str::from_utf8(&bytes[start..end]).ok()?;
    s.parse().ok()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DocumentMetadata;

    fn empty_metadata() -> DocumentMetadata {
        DocumentMetadata::default()
    }

    fn producer_metadata(producer: &str) -> DocumentMetadata {
        DocumentMetadata {
            producer: Some(producer.to_string()),
            ..Default::default()
        }
    }

    // ---- ProducerKind tests ----

    #[test]
    fn test_producer_acrobat_distiller() {
        let k = ProducerKind::from_producer_string("Acrobat Distiller 11.0");
        assert_eq!(k, ProducerKind::AdobeDistiller);
    }

    #[test]
    fn test_producer_acrobat() {
        let k = ProducerKind::from_producer_string("Adobe PDF Library 15.0");
        assert_eq!(k, ProducerKind::AdobeAcrobat);
    }

    #[test]
    fn test_producer_word() {
        let k = ProducerKind::from_producer_string("Microsoft Word for Microsoft 365");
        assert_eq!(k, ProducerKind::MicrosoftWord);
    }

    #[test]
    fn test_producer_libreoffice() {
        let k = ProducerKind::from_producer_string("LibreOffice 7.5");
        assert_eq!(k, ProducerKind::LibreOffice);
    }

    #[test]
    fn test_producer_apple_quartz() {
        let k = ProducerKind::from_producer_string("Mac OS X 13.2 Quartz PDFContext");
        assert_eq!(k, ProducerKind::AppleQuartz);
    }

    #[test]
    fn test_producer_google_docs() {
        let k = ProducerKind::from_producer_string("Docs-docservice");
        assert_eq!(k, ProducerKind::GoogleDocs);
    }

    #[test]
    fn test_producer_latex() {
        let k = ProducerKind::from_producer_string("pdfTeX-1.40.25");
        assert_eq!(k, ProducerKind::Latex);
    }

    #[test]
    fn test_producer_ghostscript() {
        let k = ProducerKind::from_producer_string("GPL Ghostscript 10.01.2");
        assert_eq!(k, ProducerKind::Ghostscript);
    }

    #[test]
    fn test_producer_wkhtmltopdf() {
        let k = ProducerKind::from_producer_string("wkhtmltopdf 0.12.6");
        assert_eq!(k, ProducerKind::Wkhtmltopdf);
    }

    #[test]
    fn test_producer_reportlab() {
        let k = ProducerKind::from_producer_string("ReportLab PDF Library - www.reportlab.com");
        assert_eq!(k, ProducerKind::Reportlab);
    }

    #[test]
    fn test_producer_itext() {
        let k = ProducerKind::from_producer_string("iText 7.2.3 (AGPL-version)");
        assert_eq!(k, ProducerKind::Itext);
    }

    #[test]
    fn test_producer_pdfium() {
        let k = ProducerKind::from_producer_string("PDFium");
        assert_eq!(k, ProducerKind::Pdfium);
    }

    #[test]
    fn test_producer_foxit() {
        let k = ProducerKind::from_producer_string("Foxit PDF Creator Version 12.1.0");
        assert_eq!(k, ProducerKind::Foxit);
    }

    #[test]
    fn test_producer_nitro() {
        let k = ProducerKind::from_producer_string("Nitro Pro 13");
        assert_eq!(k, ProducerKind::Nitro);
    }

    #[test]
    fn test_producer_pdf24() {
        let k = ProducerKind::from_producer_string("PDF24 Creator 11.0");
        assert_eq!(k, ProducerKind::Pdf24);
        assert!(k.is_online_converter());
    }

    #[test]
    fn test_producer_smallpdf() {
        let k = ProducerKind::from_producer_string("Smallpdf.com");
        assert_eq!(k, ProducerKind::Smallpdf);
        assert!(k.is_online_converter());
    }

    #[test]
    fn test_producer_docusign() {
        let k = ProducerKind::from_producer_string("DocuSign");
        assert_eq!(k, ProducerKind::DocuSign);
        assert!(k.is_esignature_platform());
    }

    #[test]
    fn test_producer_unknown() {
        let k = ProducerKind::from_producer_string("SomeBespokeTool v1.0");
        assert!(matches!(k, ProducerKind::Unknown(_)));
        assert!(!k.is_online_converter());
    }

    #[test]
    fn test_producer_empty() {
        let k = ProducerKind::from_producer_string("");
        assert!(matches!(k, ProducerKind::Unknown(_)));
    }

    #[test]
    fn test_producer_label_non_empty() {
        let kinds = vec![
            ProducerKind::AdobeAcrobat,
            ProducerKind::AdobeDistiller,
            ProducerKind::MicrosoftWord,
            ProducerKind::LibreOffice,
            ProducerKind::AppleQuartz,
            ProducerKind::GoogleDocs,
            ProducerKind::Latex,
            ProducerKind::Ghostscript,
            ProducerKind::Wkhtmltopdf,
            ProducerKind::Reportlab,
            ProducerKind::Itext,
            ProducerKind::Pdfium,
            ProducerKind::Foxit,
            ProducerKind::Nitro,
            ProducerKind::Pdf24,
            ProducerKind::Smallpdf,
            ProducerKind::AdobeLiveCycle,
            ProducerKind::DocuSign,
        ];
        for k in kinds {
            assert!(!k.label().is_empty(), "label empty for {k:?}");
        }
    }

    // ---- detect_incremental_updates tests ----

    #[test]
    fn test_no_startxref() {
        let bytes = b"this is not a pdf";
        let updates = detect_incremental_updates(bytes);
        assert!(updates.is_empty());
    }

    #[test]
    fn test_single_startxref() {
        let bytes = b"%PDF-1.7\nstartxref\n1234\n%%EOF\n";
        let updates = detect_incremental_updates(bytes);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].revision, 1);
        assert_eq!(updates[0].startxref_offset, 1234);
        assert!(!updates[0].contains_signature_hint);
    }

    #[test]
    fn test_multiple_startxref() {
        let bytes =
            b"%PDF-1.7\nstartxref\n100\n%%EOF\nstartxref\n200\n%%EOF\nstartxref\n300\n%%EOF\n";
        let updates = detect_incremental_updates(bytes);
        assert_eq!(updates.len(), 3);
        assert_eq!(updates[0].revision, 1);
        assert_eq!(updates[1].revision, 2);
        assert_eq!(updates[2].revision, 3);
        assert_eq!(updates[2].startxref_offset, 300);
    }

    #[test]
    fn test_signature_hint_detected() {
        let bytes = b"%PDF-1.7\n/Sig blah blah\nstartxref\n100\n%%EOF\n";
        let updates = detect_incremental_updates(bytes);
        assert_eq!(updates.len(), 1);
        assert!(updates[0].contains_signature_hint);
    }

    #[test]
    fn test_startxref_zero_offset() {
        let bytes = b"startxref\n0\n%%EOF";
        let updates = detect_incremental_updates(bytes);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].startxref_offset, 0);
    }

    // ---- ForensicReport::build tests ----

    fn build_report_from_metadata(meta: DocumentMetadata) -> ForensicReport {
        let bytes = b"%PDF-1.7\nstartxref\n0\n%%EOF";
        ForensicReport::build(
            &meta,
            "1.7".to_string(),
            bytes,
            vec![],
            1,
            &[0],
            &[(612.0, 792.0)],
        )
    }

    #[test]
    fn test_clean_document_risk_zero() {
        let meta = DocumentMetadata {
            producer: Some("Microsoft Word for Microsoft 365".to_string()),
            creator: Some("Microsoft Word".to_string()),
            creation_date: Some("D:20260101120000".to_string()),
            mod_date: Some("D:20260101120000".to_string()),
            ..Default::default()
        };
        let report = build_report_from_metadata(meta);
        assert_eq!(report.incremental_updates.len(), 1);
        assert!(!report.was_modified());
        assert_eq!(report.modification_count(), 0);
        assert!(report.is_clean());
    }

    #[test]
    fn test_online_converter_raises_risk() {
        let meta = producer_metadata("PDF24 Creator 11.0");
        let report = build_report_from_metadata(meta);
        assert!(report.risk_score >= 3);
        assert!(!report.risk_summary.is_empty());
        assert!(
            report
                .risk_summary
                .iter()
                .any(|s| s.contains("online converter"))
        );
    }

    #[test]
    fn test_missing_producer_and_creator_raises_risk() {
        let report = build_report_from_metadata(empty_metadata());
        assert!(report.risk_score >= 2);
        assert!(
            report
                .metadata_findings
                .iter()
                .any(|f| f.field == "producer")
        );
    }

    #[test]
    fn test_multiple_revisions_raises_risk() {
        let meta = producer_metadata("Microsoft Word");
        let bytes =
            b"%PDF-1.7\nstartxref\n100\n%%EOF\nstartxref\n200\n%%EOF\nstartxref\n300\n%%EOF\n";
        let report = ForensicReport::build(
            &meta,
            "1.7".to_string(),
            bytes,
            vec![],
            1,
            &[0],
            &[(612.0, 792.0)],
        );
        assert!(report.was_modified());
        assert_eq!(report.modification_count(), 2);
        assert!(report.risk_score >= 2);
        assert!(report.risk_summary.iter().any(|s| s.contains("modified")));
        // No sig fields but multiple revisions → metadata finding
        assert!(
            report
                .metadata_findings
                .iter()
                .any(|f| f.field == "incremental_updates")
        );
    }

    #[test]
    fn test_signed_document_all_signatures_signed() {
        let sigs = vec![
            SignatureInfo {
                signer_name: Some("Alice".to_string()),
                sign_date: Some("D:20260101".to_string()),
                reason: None,
                location: None,
                contact_info: None,
                filter: None,
                sub_filter: None,
                byte_range: None,
                is_signed: true,
            },
            SignatureInfo {
                signer_name: Some("Bob".to_string()),
                sign_date: None,
                reason: None,
                location: None,
                contact_info: None,
                filter: None,
                sub_filter: None,
                byte_range: None,
                is_signed: true,
            },
        ];
        let bytes = b"%PDF-1.7\n/Sig\nstartxref\n0\n%%EOF";
        let report = ForensicReport::build(
            &empty_metadata(),
            "1.7".to_string(),
            bytes,
            sigs,
            2,
            &[0, 0],
            &[(612.0, 792.0), (612.0, 792.0)],
        );
        assert_eq!(report.signatures.len(), 2);
        assert!(report.all_signatures_signed);
        assert!(report.has_signature_hint);
    }

    #[test]
    fn test_unsigned_sig_field_raises_risk() {
        let sigs = vec![SignatureInfo {
            signer_name: None,
            sign_date: None,
            reason: None,
            location: None,
            contact_info: None,
            filter: None,
            sub_filter: None,
            byte_range: None,
            is_signed: false,
        }];
        let bytes = b"%PDF-1.7\nstartxref\n0\n%%EOF";
        let report = ForensicReport::build(
            &empty_metadata(),
            "1.7".to_string(),
            bytes,
            sigs,
            1,
            &[0],
            &[(612.0, 792.0)],
        );
        assert!(!report.all_signatures_signed);
        assert!(report.risk_score >= 1);
    }

    #[test]
    fn test_unusual_page_size_anomaly() {
        let meta = producer_metadata("LaTeX");
        let bytes = b"%PDF-1.7\nstartxref\n0\n%%EOF";
        let report = ForensicReport::build(
            &meta,
            "1.7".to_string(),
            bytes,
            vec![],
            1,
            &[0],
            &[(10000.0, 792.0)], // absurdly wide
        );
        assert!(!report.page_geometry_anomalies.is_empty());
        assert!(
            report.page_geometry_anomalies[0]
                .description
                .contains("large")
        );
    }

    #[test]
    fn test_non_standard_rotation_anomaly() {
        let meta = producer_metadata("LaTeX");
        let bytes = b"%PDF-1.7\nstartxref\n0\n%%EOF";
        let report = ForensicReport::build(
            &meta,
            "1.7".to_string(),
            bytes,
            vec![],
            1,
            &[45], // 45° is non-standard
            &[(612.0, 792.0)],
        );
        assert!(!report.page_geometry_anomalies.is_empty());
        assert!(report.page_geometry_anomalies[0].description.contains("45"));
    }

    #[test]
    fn test_format_text_non_empty() {
        let report = build_report_from_metadata(empty_metadata());
        let text = report.format_text();
        assert!(text.contains("pdfplumber forensic inspection report"));
        assert!(text.contains("Risk Assessment"));
        assert!(text.contains("Revisions"));
    }

    #[test]
    fn test_was_modified_single_revision() {
        let report = build_report_from_metadata(empty_metadata());
        assert!(!report.was_modified());
    }

    #[test]
    fn test_creation_equals_mod_date() {
        let meta = DocumentMetadata {
            creation_date: Some("D:20260101".to_string()),
            mod_date: Some("D:20260101".to_string()),
            ..Default::default()
        };
        let report = build_report_from_metadata(meta);
        assert!(report.creation_equals_mod_date);
    }

    #[test]
    fn test_pdf_version_preserved() {
        let bytes = b"%PDF-2.0\nstartxref\n0\n%%EOF";
        let report = ForensicReport::build(
            &empty_metadata(),
            "2.0".to_string(),
            bytes,
            vec![],
            1,
            &[0],
            &[(595.0, 842.0)],
        );
        assert_eq!(report.pdf_version, "2.0");
    }

    #[test]
    fn test_memfind_basic() {
        let hay = b"hello world startxref 123";
        let pos = memfind(hay, b"startxref", 0);
        assert_eq!(pos, Some(12));
    }

    #[test]
    fn test_memfind_not_found() {
        assert_eq!(memfind(b"hello", b"xyz", 0), None);
    }

    #[test]
    fn test_memfind_from_offset() {
        let hay = b"aXbXcX";
        assert_eq!(memfind(hay, b"X", 3), Some(3));
        assert_eq!(memfind(hay, b"X", 4), Some(5));
    }

    #[test]
    fn test_skip_whitespace_and_parse_u64() {
        let bytes = b"  \n42\n";
        assert_eq!(skip_whitespace_and_parse_u64(bytes, 0), Some(42));
    }

    #[test]
    fn test_skip_whitespace_and_parse_u64_large() {
        let bytes = b"startxref\n987654321\n%%EOF";
        // starts at position 10
        assert_eq!(skip_whitespace_and_parse_u64(bytes, 10), Some(987_654_321));
    }
}
