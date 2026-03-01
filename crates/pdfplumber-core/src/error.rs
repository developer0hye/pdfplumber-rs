//! Error and warning types for pdfplumber-rs.
//!
//! Provides [`PdfError`] for fatal errors that stop processing,
//! [`ExtractWarning`] for non-fatal issues that allow best-effort continuation,
//! [`ExtractResult`] for pairing a value with collected warnings, and
//! [`ExtractOptions`] for configuring resource limits and warning behavior.

use std::fmt;

use crate::unicode_norm::UnicodeNorm;

/// Fatal error types for PDF processing.
///
/// These errors indicate conditions that prevent further processing
/// of the PDF or current operation.
#[derive(Debug, Clone, PartialEq)]
pub enum PdfError {
    /// Error parsing PDF structure or syntax.
    ParseError(String),
    /// I/O error reading PDF data.
    IoError(String),
    /// Error resolving font or encoding information.
    FontError(String),
    /// Error during content stream interpretation.
    InterpreterError(String),
    /// A configured resource limit was exceeded.
    ResourceLimitExceeded(String),
    /// The PDF is encrypted and requires a password to open.
    PasswordRequired,
    /// The supplied password is incorrect for this encrypted PDF.
    InvalidPassword,
    /// Any other error not covered by specific variants.
    Other(String),
}

impl fmt::Display for PdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PdfError::ParseError(msg) => write!(f, "parse error: {msg}"),
            PdfError::IoError(msg) => write!(f, "I/O error: {msg}"),
            PdfError::FontError(msg) => write!(f, "font error: {msg}"),
            PdfError::InterpreterError(msg) => write!(f, "interpreter error: {msg}"),
            PdfError::ResourceLimitExceeded(msg) => write!(f, "resource limit exceeded: {msg}"),
            PdfError::PasswordRequired => write!(f, "PDF is encrypted and requires a password"),
            PdfError::InvalidPassword => write!(f, "the supplied password is incorrect"),
            PdfError::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for PdfError {}

impl From<std::io::Error> for PdfError {
    fn from(err: std::io::Error) -> Self {
        PdfError::IoError(err.to_string())
    }
}

/// A non-fatal warning encountered during extraction.
///
/// Warnings allow best-effort continuation when issues are encountered
/// (e.g., missing font metrics, unknown operators). They include a
/// description and optional source location context such as page number,
/// operator index, and font name.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractWarning {
    /// Human-readable description of the warning.
    pub description: String,
    /// Page number where the warning occurred (0-indexed), if applicable.
    pub page: Option<usize>,
    /// Element context (e.g., "char at offset 42").
    pub element: Option<String>,
    /// Index of the operator in the content stream where the warning occurred.
    pub operator_index: Option<usize>,
    /// Font name associated with the warning, if applicable.
    pub font_name: Option<String>,
}

impl ExtractWarning {
    /// Create a warning with just a description.
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            page: None,
            element: None,
            operator_index: None,
            font_name: None,
        }
    }

    /// Create a warning with page context.
    pub fn on_page(description: impl Into<String>, page: usize) -> Self {
        Self {
            description: description.into(),
            page: Some(page),
            element: None,
            operator_index: None,
            font_name: None,
        }
    }

    /// Create a warning with full source context.
    pub fn with_context(
        description: impl Into<String>,
        page: usize,
        element: impl Into<String>,
    ) -> Self {
        Self {
            description: description.into(),
            page: Some(page),
            element: Some(element.into()),
            operator_index: None,
            font_name: None,
        }
    }

    /// Create a warning with operator and font context.
    ///
    /// Includes the operator index in the content stream and the font name,
    /// useful for diagnosing font-related issues during text extraction.
    pub fn with_operator_context(
        description: impl Into<String>,
        operator_index: usize,
        font_name: impl Into<String>,
    ) -> Self {
        Self {
            description: description.into(),
            page: None,
            element: None,
            operator_index: Some(operator_index),
            font_name: Some(font_name.into()),
        }
    }
}

impl fmt::Display for ExtractWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description)?;
        if let Some(page) = self.page {
            write!(f, " (page {page})")?;
        }
        if let Some(ref font_name) = self.font_name {
            write!(f, " [font {font_name}]")?;
        }
        if let Some(index) = self.operator_index {
            write!(f, " [operator #{index}]")?;
        }
        if let Some(ref element) = self.element {
            write!(f, " [{element}]")?;
        }
        Ok(())
    }
}

/// Result wrapper that pairs a value with collected warnings.
///
/// Used when extraction can partially succeed with non-fatal issues.
#[derive(Debug, Clone)]
pub struct ExtractResult<T> {
    /// The extracted value.
    pub value: T,
    /// Warnings collected during extraction.
    pub warnings: Vec<ExtractWarning>,
}

impl<T> ExtractResult<T> {
    /// Create a result with no warnings.
    pub fn ok(value: T) -> Self {
        Self {
            value,
            warnings: Vec::new(),
        }
    }

    /// Create a result with warnings.
    pub fn with_warnings(value: T, warnings: Vec<ExtractWarning>) -> Self {
        Self { value, warnings }
    }

    /// Returns true if there are no warnings.
    pub fn is_clean(&self) -> bool {
        self.warnings.is_empty()
    }

    /// Transform the value while preserving warnings.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> ExtractResult<U> {
        ExtractResult {
            value: f(self.value),
            warnings: self.warnings,
        }
    }
}

/// Options controlling extraction behavior and resource limits.
///
/// Provides sensible defaults for all settings. Resource limits prevent
/// pathological PDFs from consuming excessive memory or causing infinite loops.
#[derive(Debug, Clone)]
pub struct ExtractOptions {
    /// Maximum recursion depth for nested Form XObjects (default: 10).
    pub max_recursion_depth: usize,
    /// Maximum number of objects extracted per page (default: 100,000).
    pub max_objects_per_page: usize,
    /// Maximum content stream bytes to process (default: 100 MB).
    pub max_stream_bytes: usize,
    /// Whether to collect warnings during extraction (default: true).
    pub collect_warnings: bool,
    /// Unicode normalization form to apply to extracted character text (default: Nfc).
    pub unicode_norm: UnicodeNorm,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            max_recursion_depth: 10,
            max_objects_per_page: 100_000,
            max_stream_bytes: 100 * 1024 * 1024,
            collect_warnings: true,
            unicode_norm: UnicodeNorm::Nfc,
        }
    }
}

impl ExtractOptions {
    /// Create options optimized for LLM consumption.
    ///
    /// Returns options with NFC Unicode normalization enabled, which ensures
    /// consistent text representation for language model processing.
    pub fn for_llm() -> Self {
        Self {
            unicode_norm: UnicodeNorm::Nfc,
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unicode_norm::UnicodeNorm;

    // --- PdfError tests ---

    #[test]
    fn pdf_error_parse_error_creation() {
        let err = PdfError::ParseError("invalid xref".to_string());
        assert_eq!(err.to_string(), "parse error: invalid xref");
    }

    #[test]
    fn pdf_error_io_error_creation() {
        let err = PdfError::IoError("file not found".to_string());
        assert_eq!(err.to_string(), "I/O error: file not found");
    }

    #[test]
    fn pdf_error_font_error_creation() {
        let err = PdfError::FontError("missing glyph widths".to_string());
        assert_eq!(err.to_string(), "font error: missing glyph widths");
    }

    #[test]
    fn pdf_error_interpreter_error_creation() {
        let err = PdfError::InterpreterError("unknown operator".to_string());
        assert_eq!(err.to_string(), "interpreter error: unknown operator");
    }

    #[test]
    fn pdf_error_resource_limit_exceeded() {
        let err = PdfError::ResourceLimitExceeded("too many objects".to_string());
        assert_eq!(err.to_string(), "resource limit exceeded: too many objects");
    }

    #[test]
    fn pdf_error_password_required() {
        let err = PdfError::PasswordRequired;
        assert_eq!(err.to_string(), "PDF is encrypted and requires a password");
    }

    #[test]
    fn pdf_error_invalid_password() {
        let err = PdfError::InvalidPassword;
        assert_eq!(err.to_string(), "the supplied password is incorrect");
    }

    #[test]
    fn pdf_error_password_required_clone_and_eq() {
        let err1 = PdfError::PasswordRequired;
        let err2 = err1.clone();
        assert_eq!(err1, err2);
    }

    #[test]
    fn pdf_error_invalid_password_clone_and_eq() {
        let err1 = PdfError::InvalidPassword;
        let err2 = err1.clone();
        assert_eq!(err1, err2);
    }

    #[test]
    fn pdf_error_other() {
        let err = PdfError::Other("something went wrong".to_string());
        assert_eq!(err.to_string(), "something went wrong");
    }

    #[test]
    fn pdf_error_implements_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(PdfError::ParseError("test".to_string()));
        assert_eq!(err.to_string(), "parse error: test");
    }

    #[test]
    fn pdf_error_clone_and_eq() {
        let err1 = PdfError::ParseError("test".to_string());
        let err2 = err1.clone();
        assert_eq!(err1, err2);
    }

    #[test]
    fn pdf_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing file");
        let pdf_err: PdfError = io_err.into();
        assert!(matches!(pdf_err, PdfError::IoError(_)));
        assert!(pdf_err.to_string().contains("missing file"));
    }

    // --- ExtractWarning tests ---

    #[test]
    fn warning_new_with_description_only() {
        let w = ExtractWarning::new("missing font metrics");
        assert_eq!(w.description, "missing font metrics");
        assert_eq!(w.page, None);
        assert_eq!(w.element, None);
        assert_eq!(w.operator_index, None);
        assert_eq!(w.font_name, None);
        assert_eq!(w.to_string(), "missing font metrics");
    }

    #[test]
    fn warning_on_page() {
        let w = ExtractWarning::on_page("unknown operator", 3);
        assert_eq!(w.description, "unknown operator");
        assert_eq!(w.page, Some(3));
        assert_eq!(w.element, None);
        assert_eq!(w.operator_index, None);
        assert_eq!(w.font_name, None);
        assert_eq!(w.to_string(), "unknown operator (page 3)");
    }

    #[test]
    fn warning_with_full_context() {
        let w = ExtractWarning::with_context("missing width", 1, "char at offset 42");
        assert_eq!(w.description, "missing width");
        assert_eq!(w.page, Some(1));
        assert_eq!(w.element, Some("char at offset 42".to_string()));
        assert_eq!(w.operator_index, None);
        assert_eq!(w.font_name, None);
        assert_eq!(w.to_string(), "missing width (page 1) [char at offset 42]");
    }

    #[test]
    fn warning_with_operator_context() {
        let w =
            ExtractWarning::with_operator_context("font not found in resources", 5, "Helvetica");
        assert_eq!(w.description, "font not found in resources");
        assert_eq!(w.page, None);
        assert_eq!(w.element, None);
        assert_eq!(w.operator_index, Some(5));
        assert_eq!(w.font_name, Some("Helvetica".to_string()));
        assert_eq!(
            w.to_string(),
            "font not found in resources [font Helvetica] [operator #5]"
        );
    }

    #[test]
    fn warning_display_with_all_fields() {
        let w = ExtractWarning {
            description: "test warning".to_string(),
            page: Some(2),
            element: Some("extra context".to_string()),
            operator_index: Some(10),
            font_name: Some("Arial".to_string()),
        };
        assert_eq!(
            w.to_string(),
            "test warning (page 2) [font Arial] [operator #10] [extra context]"
        );
    }

    #[test]
    fn warning_clone_and_eq() {
        let w1 = ExtractWarning::on_page("test warning", 0);
        let w2 = w1.clone();
        assert_eq!(w1, w2);
    }

    #[test]
    fn warning_with_operator_context_clone_and_eq() {
        let w1 = ExtractWarning::with_operator_context("test", 3, "Times");
        let w2 = w1.clone();
        assert_eq!(w1, w2);
    }

    // --- ExtractResult tests ---

    #[test]
    fn extract_result_ok_no_warnings() {
        let result = ExtractResult::ok(42);
        assert_eq!(result.value, 42);
        assert!(result.warnings.is_empty());
        assert!(result.is_clean());
    }

    #[test]
    fn extract_result_with_warnings() {
        let warnings = vec![
            ExtractWarning::new("warn 1"),
            ExtractWarning::on_page("warn 2", 0),
        ];
        let result = ExtractResult::with_warnings("hello", warnings);
        assert_eq!(result.value, "hello");
        assert_eq!(result.warnings.len(), 2);
        assert!(!result.is_clean());
    }

    #[test]
    fn extract_result_map_preserves_warnings() {
        let warnings = vec![ExtractWarning::new("test")];
        let result = ExtractResult::with_warnings(10, warnings);
        let mapped = result.map(|v| v * 2);
        assert_eq!(mapped.value, 20);
        assert_eq!(mapped.warnings.len(), 1);
        assert_eq!(mapped.warnings[0].description, "test");
    }

    #[test]
    fn extract_result_collect_multiple_warnings() {
        let mut result = ExtractResult::ok(Vec::<String>::new());
        result.warnings.push(ExtractWarning::new("first"));
        result.warnings.push(ExtractWarning::on_page("second", 1));
        result
            .warnings
            .push(ExtractWarning::with_context("third", 2, "char 'A'"));
        assert_eq!(result.warnings.len(), 3);
    }

    // --- ExtractOptions tests ---

    #[test]
    fn extract_options_default_values() {
        let opts = ExtractOptions::default();
        assert_eq!(opts.max_recursion_depth, 10);
        assert_eq!(opts.max_objects_per_page, 100_000);
        assert_eq!(opts.max_stream_bytes, 100 * 1024 * 1024);
        assert!(opts.collect_warnings);
        assert_eq!(opts.unicode_norm, UnicodeNorm::Nfc);
    }

    #[test]
    fn extract_options_for_llm() {
        let opts = ExtractOptions::for_llm();
        assert_eq!(opts.unicode_norm, UnicodeNorm::Nfc);
        assert_eq!(opts.max_recursion_depth, 10);
        assert_eq!(opts.max_objects_per_page, 100_000);
        assert_eq!(opts.max_stream_bytes, 100 * 1024 * 1024);
        assert!(opts.collect_warnings);
    }

    #[test]
    fn extract_options_custom_values() {
        let opts = ExtractOptions {
            max_recursion_depth: 5,
            max_objects_per_page: 50_000,
            max_stream_bytes: 10 * 1024 * 1024,
            collect_warnings: false,
            unicode_norm: UnicodeNorm::None,
        };
        assert_eq!(opts.max_recursion_depth, 5);
        assert_eq!(opts.max_objects_per_page, 50_000);
        assert_eq!(opts.max_stream_bytes, 10 * 1024 * 1024);
        assert!(!opts.collect_warnings);
    }

    #[test]
    fn extract_options_clone() {
        let opts1 = ExtractOptions::default();
        let opts2 = opts1.clone();
        assert_eq!(opts2.max_recursion_depth, opts1.max_recursion_depth);
        assert_eq!(opts2.collect_warnings, opts1.collect_warnings);
    }
}
