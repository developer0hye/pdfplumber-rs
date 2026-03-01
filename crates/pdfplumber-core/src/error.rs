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
    ResourceLimitExceeded {
        /// Name of the limit that was exceeded (e.g., "max_input_bytes").
        limit_name: String,
        /// The configured limit value.
        limit_value: usize,
        /// The actual value that exceeded the limit.
        actual_value: usize,
    },
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
            PdfError::ResourceLimitExceeded {
                limit_name,
                limit_value,
                actual_value,
            } => write!(
                f,
                "resource limit exceeded: {limit_name} (limit: {limit_value}, actual: {actual_value})"
            ),
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

/// Machine-readable warning code for categorizing extraction issues.
///
/// Each variant represents a specific category of non-fatal issue that
/// can occur during PDF extraction. Use [`Other`](ExtractWarningCode::Other)
/// for custom or uncategorized warnings.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(tag = "type", content = "detail")
)]
pub enum ExtractWarningCode {
    /// A referenced font was not found in page resources.
    MissingFont,
    /// An unsupported PDF content stream operator was encountered.
    UnsupportedOperator,
    /// A PDF object is malformed or has unexpected structure.
    MalformedObject,
    /// A configured resource limit was reached during extraction.
    ResourceLimitReached,
    /// Character encoding fell back to a default mapping.
    EncodingFallback,
    /// Any other warning not covered by specific variants.
    Other(String),
}

impl ExtractWarningCode {
    /// Returns the string tag for this warning code.
    pub fn as_str(&self) -> &str {
        match self {
            ExtractWarningCode::MissingFont => "MISSING_FONT",
            ExtractWarningCode::UnsupportedOperator => "UNSUPPORTED_OPERATOR",
            ExtractWarningCode::MalformedObject => "MALFORMED_OBJECT",
            ExtractWarningCode::ResourceLimitReached => "RESOURCE_LIMIT_REACHED",
            ExtractWarningCode::EncodingFallback => "ENCODING_FALLBACK",
            ExtractWarningCode::Other(_) => "OTHER",
        }
    }
}

impl fmt::Display for ExtractWarningCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A non-fatal warning encountered during extraction.
///
/// Warnings allow best-effort continuation when issues are encountered
/// (e.g., missing font metrics, unknown operators). They include a
/// structured [`code`](ExtractWarning::code), a human-readable description,
/// and optional source location context such as page number, operator index,
/// and font name.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExtractWarning {
    /// Machine-readable warning code.
    pub code: ExtractWarningCode,
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
    ///
    /// Uses [`ExtractWarningCode::Other`] as the default code.
    pub fn new(description: impl Into<String>) -> Self {
        let desc = description.into();
        Self {
            code: ExtractWarningCode::Other(desc.clone()),
            description: desc,
            page: None,
            element: None,
            operator_index: None,
            font_name: None,
        }
    }

    /// Create a warning with a specific code and description.
    pub fn with_code(code: ExtractWarningCode, description: impl Into<String>) -> Self {
        Self {
            code,
            description: description.into(),
            page: None,
            element: None,
            operator_index: None,
            font_name: None,
        }
    }

    /// Create a warning with page context.
    pub fn on_page(description: impl Into<String>, page: usize) -> Self {
        let desc = description.into();
        Self {
            code: ExtractWarningCode::Other(desc.clone()),
            description: desc,
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
        let desc = description.into();
        Self {
            code: ExtractWarningCode::Other(desc.clone()),
            description: desc,
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
        let desc = description.into();
        Self {
            code: ExtractWarningCode::Other(desc.clone()),
            description: desc,
            page: None,
            element: None,
            operator_index: Some(operator_index),
            font_name: Some(font_name.into()),
        }
    }

    /// Set the warning code, returning the modified warning (builder pattern).
    pub fn set_code(mut self, code: ExtractWarningCode) -> Self {
        self.code = code;
        self
    }

    /// Convert this warning into a [`PdfError`].
    ///
    /// Used by strict mode to escalate warnings to errors.
    pub fn to_error(&self) -> PdfError {
        PdfError::Other(self.to_string())
    }
}

impl fmt::Display for ExtractWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.description)?;
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
    /// Whether to extract image stream data into Image structs (default: false).
    ///
    /// When enabled, each `Image` will have its `data`, `filter`, and `mime_type`
    /// fields populated with the raw stream bytes and encoding information.
    /// Disabled by default to avoid memory overhead for large images.
    pub extract_image_data: bool,
    /// When true, any warning is escalated to an error (default: false).
    pub strict_mode: bool,
    /// Maximum input PDF file size in bytes (default: None = no limit).
    pub max_input_bytes: Option<usize>,
    /// Maximum number of pages to process (default: None = no limit).
    pub max_pages: Option<usize>,
    /// Maximum total image bytes across all pages (default: None = no limit).
    pub max_total_image_bytes: Option<usize>,
    /// Maximum total extracted objects across all pages (default: None = no limit).
    pub max_total_objects: Option<usize>,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            max_recursion_depth: 10,
            max_objects_per_page: 100_000,
            max_stream_bytes: 100 * 1024 * 1024,
            collect_warnings: true,
            unicode_norm: UnicodeNorm::Nfc,
            extract_image_data: false,
            strict_mode: false,
            max_input_bytes: None,
            max_pages: None,
            max_total_image_bytes: None,
            max_total_objects: None,
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
        let err = PdfError::ResourceLimitExceeded {
            limit_name: "max_input_bytes".to_string(),
            limit_value: 1024,
            actual_value: 2048,
        };
        assert_eq!(
            err.to_string(),
            "resource limit exceeded: max_input_bytes (limit: 1024, actual: 2048)"
        );
    }

    #[test]
    fn pdf_error_resource_limit_exceeded_structured_fields() {
        let err = PdfError::ResourceLimitExceeded {
            limit_name: "max_pages".to_string(),
            limit_value: 10,
            actual_value: 25,
        };
        if let PdfError::ResourceLimitExceeded {
            limit_name,
            limit_value,
            actual_value,
        } = &err
        {
            assert_eq!(limit_name, "max_pages");
            assert_eq!(*limit_value, 10);
            assert_eq!(*actual_value, 25);
        } else {
            panic!("expected ResourceLimitExceeded");
        }
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
        assert!(matches!(w.code, ExtractWarningCode::Other(_)));
        assert_eq!(w.page, None);
        assert_eq!(w.element, None);
        assert_eq!(w.operator_index, None);
        assert_eq!(w.font_name, None);
        assert_eq!(w.to_string(), "[OTHER] missing font metrics");
    }

    #[test]
    fn warning_on_page() {
        let w = ExtractWarning::on_page("unknown operator", 3);
        assert_eq!(w.description, "unknown operator");
        assert_eq!(w.page, Some(3));
        assert_eq!(w.element, None);
        assert_eq!(w.operator_index, None);
        assert_eq!(w.font_name, None);
        assert_eq!(w.to_string(), "[OTHER] unknown operator (page 3)");
    }

    #[test]
    fn warning_with_full_context() {
        let w = ExtractWarning::with_context("missing width", 1, "char at offset 42");
        assert_eq!(w.description, "missing width");
        assert_eq!(w.page, Some(1));
        assert_eq!(w.element, Some("char at offset 42".to_string()));
        assert_eq!(w.operator_index, None);
        assert_eq!(w.font_name, None);
        assert_eq!(
            w.to_string(),
            "[OTHER] missing width (page 1) [char at offset 42]"
        );
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
            "[OTHER] font not found in resources [font Helvetica] [operator #5]"
        );
    }

    #[test]
    fn warning_display_with_all_fields() {
        let w = ExtractWarning {
            code: ExtractWarningCode::MissingFont,
            description: "test warning".to_string(),
            page: Some(2),
            element: Some("extra context".to_string()),
            operator_index: Some(10),
            font_name: Some("Arial".to_string()),
        };
        assert_eq!(
            w.to_string(),
            "[MISSING_FONT] test warning (page 2) [font Arial] [operator #10] [extra context]"
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
        assert!(!opts.extract_image_data);
        assert!(opts.max_input_bytes.is_none());
        assert!(opts.max_pages.is_none());
        assert!(opts.max_total_image_bytes.is_none());
        assert!(opts.max_total_objects.is_none());
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
            extract_image_data: true,
            strict_mode: true,
            max_input_bytes: Some(1024),
            max_pages: Some(10),
            max_total_image_bytes: Some(5 * 1024 * 1024),
            max_total_objects: Some(100_000),
        };
        assert_eq!(opts.max_recursion_depth, 5);
        assert_eq!(opts.max_objects_per_page, 50_000);
        assert_eq!(opts.max_stream_bytes, 10 * 1024 * 1024);
        assert!(!opts.collect_warnings);
        assert!(opts.extract_image_data);
        assert!(opts.strict_mode);
        assert_eq!(opts.max_input_bytes, Some(1024));
        assert_eq!(opts.max_pages, Some(10));
        assert_eq!(opts.max_total_image_bytes, Some(5 * 1024 * 1024));
        assert_eq!(opts.max_total_objects, Some(100_000));
    }

    #[test]
    fn extract_options_clone() {
        let opts1 = ExtractOptions::default();
        let opts2 = opts1.clone();
        assert_eq!(opts2.max_recursion_depth, opts1.max_recursion_depth);
        assert_eq!(opts2.collect_warnings, opts1.collect_warnings);
    }

    // --- US-096: ExtractWarningCode tests ---

    #[test]
    fn warning_code_missing_font() {
        let code = ExtractWarningCode::MissingFont;
        assert_eq!(code.as_str(), "MISSING_FONT");
    }

    #[test]
    fn warning_code_unsupported_operator() {
        let code = ExtractWarningCode::UnsupportedOperator;
        assert_eq!(code.as_str(), "UNSUPPORTED_OPERATOR");
    }

    #[test]
    fn warning_code_malformed_object() {
        let code = ExtractWarningCode::MalformedObject;
        assert_eq!(code.as_str(), "MALFORMED_OBJECT");
    }

    #[test]
    fn warning_code_resource_limit_reached() {
        let code = ExtractWarningCode::ResourceLimitReached;
        assert_eq!(code.as_str(), "RESOURCE_LIMIT_REACHED");
    }

    #[test]
    fn warning_code_encoding_fallback() {
        let code = ExtractWarningCode::EncodingFallback;
        assert_eq!(code.as_str(), "ENCODING_FALLBACK");
    }

    #[test]
    fn warning_code_other_preserves_custom_message() {
        let code = ExtractWarningCode::Other("custom issue".to_string());
        assert_eq!(code.as_str(), "OTHER");
        if let ExtractWarningCode::Other(msg) = &code {
            assert_eq!(msg, "custom issue");
        } else {
            panic!("expected Other variant");
        }
    }

    #[test]
    fn warning_code_clone_and_eq() {
        let code1 = ExtractWarningCode::MissingFont;
        let code2 = code1.clone();
        assert_eq!(code1, code2);

        let code3 = ExtractWarningCode::Other("test".to_string());
        let code4 = code3.clone();
        assert_eq!(code3, code4);
    }

    #[test]
    fn warning_with_code_field() {
        let w = ExtractWarning::new("missing font metrics");
        // new() should default to Other code
        assert!(matches!(w.code, ExtractWarningCode::Other(_)));
    }

    #[test]
    fn warning_with_explicit_code() {
        let w = ExtractWarning {
            code: ExtractWarningCode::MissingFont,
            description: "font not found".to_string(),
            page: Some(0),
            element: None,
            operator_index: None,
            font_name: None,
        };
        assert_eq!(w.code, ExtractWarningCode::MissingFont);
        assert_eq!(w.page, Some(0));
    }

    #[test]
    fn warning_display_format_with_code() {
        let w = ExtractWarning {
            code: ExtractWarningCode::MissingFont,
            description: "font not found".to_string(),
            page: Some(2),
            element: None,
            operator_index: None,
            font_name: None,
        };
        assert_eq!(w.to_string(), "[MISSING_FONT] font not found (page 2)");
    }

    #[test]
    fn warning_display_format_with_code_no_page() {
        let w = ExtractWarning {
            code: ExtractWarningCode::UnsupportedOperator,
            description: "unknown op".to_string(),
            page: None,
            element: None,
            operator_index: None,
            font_name: None,
        };
        assert_eq!(w.to_string(), "[UNSUPPORTED_OPERATOR] unknown op");
    }

    #[test]
    fn warning_display_format_other_code() {
        let w = ExtractWarning {
            code: ExtractWarningCode::Other("custom".to_string()),
            description: "something happened".to_string(),
            page: Some(5),
            element: None,
            operator_index: None,
            font_name: None,
        };
        assert_eq!(w.to_string(), "[OTHER] something happened (page 5)");
    }

    #[test]
    fn strict_mode_default_false() {
        let opts = ExtractOptions::default();
        assert!(!opts.strict_mode);
    }

    #[test]
    fn strict_mode_converts_warning_to_error() {
        let warning = ExtractWarning {
            code: ExtractWarningCode::MissingFont,
            description: "font not found".to_string(),
            page: Some(0),
            element: None,
            operator_index: None,
            font_name: None,
        };
        let err: PdfError = warning.to_error();
        assert!(matches!(err, PdfError::Other(_)));
        assert!(err.to_string().contains("font not found"));
    }

    #[test]
    fn warning_code_display() {
        assert_eq!(
            format!("{}", ExtractWarningCode::MissingFont),
            "MISSING_FONT"
        );
        assert_eq!(
            format!("{}", ExtractWarningCode::Other("x".into())),
            "OTHER"
        );
    }

    // --- US-097: Document-level resource budgets ---

    #[test]
    fn resource_budget_defaults_none() {
        let opts = ExtractOptions::default();
        assert!(opts.max_input_bytes.is_none());
        assert!(opts.max_pages.is_none());
        assert!(opts.max_total_image_bytes.is_none());
        assert!(opts.max_total_objects.is_none());
    }

    #[test]
    fn resource_budget_custom_values() {
        let opts = ExtractOptions {
            max_input_bytes: Some(1024 * 1024),
            max_pages: Some(50),
            max_total_image_bytes: Some(10 * 1024 * 1024),
            max_total_objects: Some(500_000),
            ..ExtractOptions::default()
        };
        assert_eq!(opts.max_input_bytes, Some(1024 * 1024));
        assert_eq!(opts.max_pages, Some(50));
        assert_eq!(opts.max_total_image_bytes, Some(10 * 1024 * 1024));
        assert_eq!(opts.max_total_objects, Some(500_000));
    }

    #[test]
    fn resource_limit_exceeded_clone_and_eq() {
        let err1 = PdfError::ResourceLimitExceeded {
            limit_name: "max_input_bytes".to_string(),
            limit_value: 100,
            actual_value: 200,
        };
        let err2 = err1.clone();
        assert_eq!(err1, err2);
    }
}
