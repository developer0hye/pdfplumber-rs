//! PDF/UA accessibility analysis for pdfplumber-rs.
//!
//! Analyses a PDF document against the **PDF/UA-1** (ISO 14289-1:2014)
//! accessibility standard and reports violations. Can also infer and
//! emit a corrected structure tree for untagged documents.
//!
//! # Why this exists
//!
//! The EU Accessibility Act (2025) requires PDF/UA compliance for public-sector
//! and increasingly private-sector documents across the EU. Adobe Acrobat Pro
//! is currently the primary tool for auto-tagging at $240/year per seat.
//! This crate provides the same analysis free, offline, and embeddable.
//!
//! # Quick start
//!
//! ```no_run
//! use pdfplumber::Pdf;
//! use pdfplumber_a11y::{A11yAnalyzer, A11yReport};
//!
//! let pdf = Pdf::open_file("document.pdf", None).unwrap();
//! let report = A11yAnalyzer::new().analyze(&pdf);
//!
//! println!("PDF/UA compliant: {}", report.is_compliant());
//! for violation in report.violations() {
//!     println!("[{}] {}", violation.rule_id(), violation.message());
//! }
//! ```
//!
//! # Validation rules
//!
//! The analyzer checks the following PDF/UA-1 requirements:
//!
//! | Rule ID | Requirement |
//! |---------|-------------|
//! | UA-001  | Document must be tagged (`/Marked true` in MarkInfo dict) |
//! | UA-002  | All structure elements must use standard tag names or role maps |
//! | UA-003  | Figures must have `/Alt` text or `/ActualText` |
//! | UA-004  | Tables must have header cells (`TH`) with scope attributes |
//! | UA-005  | Document must have a `/Lang` entry specifying the natural language |
//! | UA-006  | Headings must be nested in logical order (H1 before H2, etc.) |
//! | UA-007  | All content must be tagged — no untagged real content |
//! | UA-008  | Document metadata: /Title must be non-empty |
//! | UA-009  | Artifacts must be marked as artifacts, not included in reading order |
//! | UA-010  | Links must have `/Alt` text or visible text content |
//!
//! Rules are based on the Matterhorn Protocol (PDF/UA conformance checker spec).

#![deny(missing_docs)]

mod rules;
mod tag_infer;

pub use rules::{A11yAnalyzer, A11yReport, Severity, Violation};
pub use tag_infer::{InferredTag, TagInferrer};
