//! Watermark detection and forensic metadata inspection for pdfplumber-rs.
//!
//! Answers the question: *"Is this document what it claims to be, and has it
//! been tampered with?"* Useful for law firms, journalists, FOIA researchers,
//! and anyone who receives a PDF and needs to establish provenance.
//!
//! This crate is a high-level wrapper around the lower-level forensic
//! primitives in `pdfplumber-core::forensic`. It provides:
//!
//! - **Rich metadata extraction**: all XMP/DocInfo fields, PDF version,
//!   producer, creator, linearization, encryption status, incremental updates
//! - **Watermark re-export**: transparent text layers, repeated low-opacity
//!   content, invisible text (`text rendering mode 3`), white-on-white text
//! - **Incremental update detection**: were objects added/modified after the
//!   original creation? Are digital signatures bypassed?
//! - **Anomaly scoring**: a machine-readable score (0–100) indicating how
//!   suspicious the document is
//! - **Formatted reports**: human-readable multi-line summaries for CLI / audit
//!
//! # Quick start
//!
//! ```no_run
//! use pdfplumber_forensic::ForensicInspector;
//!
//! let bytes = std::fs::read("contract.pdf").unwrap();
//! let report = ForensicInspector::inspect_bytes(&bytes).unwrap();
//!
//! println!("{}", report.summary());
//! if report.anomaly_score() > 50 {
//!     println!("WARNING: document has suspicious characteristics");
//! }
//! ```
//!
//! # Using with an already-open Pdf
//!
//! ```no_run
//! use pdfplumber::Pdf;
//! use pdfplumber_forensic::ForensicInspector;
//!
//! let bytes = std::fs::read("contract.pdf").unwrap();
//! let pdf = Pdf::open(&bytes, None).unwrap();
//! let report = ForensicInspector::inspect(&pdf, &bytes);
//!
//! println!("{}", report.summary());
//! ```

#![deny(missing_docs)]

mod anomaly;
mod metadata;

// Re-export core types so downstream crates don't need to depend on
// pdfplumber-core just to inspect forensic findings.
pub use pdfplumber_core::{
    ForensicReport as CoreForensicReport, IncrementalUpdate, MetadataFinding, PageGeometryAnomaly,
    ProducerKind, WatermarkFinding, WatermarkKind,
};

pub use anomaly::{ForensicInspector, ForensicSummary};
pub use metadata::ExtendedMetadata;
