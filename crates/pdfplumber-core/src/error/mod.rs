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
///
/// This enum is `#[non_exhaustive]` — new variants may be added in minor
/// releases as new PDF features and error categories are recognised. Match
/// against it with a `_ =>` arm or use the typed accessors.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
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
///
/// This enum is `#[non_exhaustive]` — new warning categories will be added
/// as the library gains coverage of more PDF constructs.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
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
    /// Character deduplication options (default: enabled with tolerance 1.0).
    ///
    /// When `Some`, duplicate overlapping characters are removed after extraction.
    /// Some PDF generators output duplicate glyphs for bold/shadow effects or
    /// due to bugs. Set to `None` to disable deduplication.
    pub dedupe: Option<crate::dedupe::DedupeOptions>,
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
            dedupe: Some(crate::dedupe::DedupeOptions::default()),
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
mod tests;
