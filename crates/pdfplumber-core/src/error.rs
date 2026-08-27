//! Error and warning types for pdfplumber-rs.
//!
//! Provides [`PdfError`] for fatal errors that stop processing,
//! [`ExtractWarning`] for non-fatal issues that allow best-effort continuation,
//! [`ExtractResult`] for pairing a value with collected warnings, and
//! [`ExtractOptions`] for configuring resource limits and warning behavior.

use std::fmt;

use crate::unicode_norm::UnicodeNorm;

/// Machine-readable category for a fatal PDF operation error.
///
/// Match this non-exhaustive enum with a wildcard so future error categories
/// can be added without breaking callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PdfErrorKind {
    /// PDF structure, syntax, or object resolution is invalid.
    Parse,
    /// An input or output operation failed.
    Io,
    /// Font or encoding information could not be resolved.
    Font,
    /// A page content stream could not be interpreted.
    Interpreter,
    /// A configured resource limit was exceeded.
    ResourceLimit,
    /// The PDF requires a password.
    PasswordRequired,
    /// The supplied password is incorrect.
    InvalidPassword,
    /// A failure outside the other public categories occurred.
    Other,
}

impl PdfErrorKind {
    /// Return the stable, machine-readable code for this error category.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Parse => "PARSE",
            Self::Io => "IO",
            Self::Font => "FONT",
            Self::Interpreter => "INTERPRETER",
            Self::ResourceLimit => "RESOURCE_LIMIT",
            Self::PasswordRequired => "PASSWORD_REQUIRED",
            Self::InvalidPassword => "INVALID_PASSWORD",
            Self::Other => "OTHER",
        }
    }
}

/// A PDF indirect object identifier attached to an error when known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PdfObjectId {
    /// Indirect object number.
    pub number: u32,
    /// Indirect object generation number.
    pub generation: u16,
}

/// Safe location and operation context attached to a [`PdfError`].
///
/// Page indices are zero-based. The operation names are library-owned static
/// labels; input paths and document text are never stored here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct PdfErrorContext {
    /// Library operation that failed, if the facade knew it.
    pub operation: Option<&'static str>,
    /// Zero-based page index involved in the failure, if known.
    pub page_index: Option<usize>,
    /// PDF indirect object involved in the failure, if known.
    pub object_id: Option<PdfObjectId>,
}

/// Machine-readable details for a configured resource-limit failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct PdfResourceLimit {
    /// Name of the configured limit, such as `max_input_bytes`.
    pub name: &'static str,
    /// Configured maximum value.
    pub limit: usize,
    /// Observed value that proved the limit was exceeded.
    pub observed: usize,
}

/// Fatal error returned by the public Rust facade.
///
/// The representation is intentionally opaque. Use [`PdfError::kind`],
/// [`PdfError::context`], and [`PdfError::resource_limit`] for stable typed
/// inspection, and [`std::error::Error::source`] for opt-in diagnostics. The
/// default [`fmt::Display`] and [`fmt::Debug`] output never renders the source,
/// so input paths and document content are not disclosed by ordinary logging.
pub struct PdfError {
    kind: PdfErrorKind,
    context: PdfErrorContext,
    resource_limit: Option<PdfResourceLimit>,
    source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
}

impl PdfError {
    /// Create an error category without an underlying source.
    pub fn new(kind: PdfErrorKind) -> Self {
        Self {
            kind,
            context: PdfErrorContext::default(),
            resource_limit: None,
            source: None,
        }
    }

    /// Create an error category while preserving its underlying cause.
    pub fn from_source<E>(kind: PdfErrorKind, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self {
            kind,
            context: PdfErrorContext::default(),
            resource_limit: None,
            source: Some(Box::new(source)),
        }
    }

    /// Create a parse error from an existing diagnostic message.
    pub fn parse(message: impl Into<String>) -> Self {
        Self::from_source(PdfErrorKind::Parse, MessageError(message.into()))
    }

    /// Create a font error from an existing diagnostic message.
    pub fn font(message: impl Into<String>) -> Self {
        Self::from_source(PdfErrorKind::Font, MessageError(message.into()))
    }

    /// Create an interpreter error from an existing diagnostic message.
    pub fn interpreter(message: impl Into<String>) -> Self {
        Self::from_source(PdfErrorKind::Interpreter, MessageError(message.into()))
    }

    /// Create an uncategorized error from an existing diagnostic message.
    pub fn other(message: impl Into<String>) -> Self {
        Self::from_source(PdfErrorKind::Other, MessageError(message.into()))
    }

    /// Create a configured resource-limit error.
    pub fn limit_exceeded(name: &'static str, limit: usize, observed: usize) -> Self {
        Self {
            kind: PdfErrorKind::ResourceLimit,
            context: PdfErrorContext::default(),
            resource_limit: Some(PdfResourceLimit {
                name,
                limit,
                observed,
            }),
            source: None,
        }
    }

    /// Create an error indicating that a password is required.
    pub fn password_required() -> Self {
        Self::new(PdfErrorKind::PasswordRequired)
    }

    /// Create an error indicating that the supplied password is invalid.
    pub fn invalid_password() -> Self {
        Self::new(PdfErrorKind::InvalidPassword)
    }

    /// Return this error's stable category.
    pub const fn kind(&self) -> PdfErrorKind {
        self.kind
    }

    /// Return safe operation, page, and object context.
    pub const fn context(&self) -> &PdfErrorContext {
        &self.context
    }

    /// Return typed resource-limit details when this is a limit error.
    pub const fn resource_limit(&self) -> Option<&PdfResourceLimit> {
        self.resource_limit.as_ref()
    }

    /// Attach a library-owned operation label.
    pub fn during(mut self, operation: &'static str) -> Self {
        self.context.operation = Some(operation);
        self
    }

    /// Attach a zero-based page index.
    pub fn at_page(mut self, page_index: usize) -> Self {
        self.context.page_index = Some(page_index);
        self
    }

    /// Attach a PDF indirect object identifier.
    pub fn at_object(mut self, number: u32, generation: u16) -> Self {
        self.context.object_id = Some(PdfObjectId { number, generation });
        self
    }
}

impl fmt::Display for PdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            PdfErrorKind::Parse => f.write_str(
                "could not parse PDF input; verify that the input is a valid, supported PDF",
            )?,
            PdfErrorKind::Io => f.write_str(
                "could not read PDF input; check that the source is available and readable",
            )?,
            PdfErrorKind::Font => f.write_str(
                "could not resolve PDF font data; inspect the source chain for font details",
            )?,
            PdfErrorKind::Interpreter => f.write_str(
                "could not interpret PDF page content; inspect the reported page or object context",
            )?,
            PdfErrorKind::ResourceLimit => {
                if let Some(limit) = &self.resource_limit {
                    write!(
                        f,
                        "resource limit {} exceeded (limit {}, observed {}); raise the configured limit or use a smaller input",
                        limit.name, limit.limit, limit.observed
                    )?;
                } else {
                    f.write_str(
                        "a configured resource limit was exceeded; raise the limit or use a smaller input",
                    )?;
                }
            }
            PdfErrorKind::PasswordRequired => {
                f.write_str("PDF is encrypted; retry with one of the password-aware open methods")?
            }
            PdfErrorKind::InvalidPassword => {
                f.write_str("could not decrypt PDF; verify the supplied password")?;
            }
            PdfErrorKind::Other => {
                f.write_str("PDF operation failed; inspect the source chain for details")?;
            }
        }

        let mut wrote_context = false;
        if let Some(operation) = self.context.operation {
            write!(f, " [operation: {operation}")?;
            wrote_context = true;
        }
        if let Some(page_index) = self.context.page_index {
            if wrote_context {
                write!(f, ", page index {page_index}")?;
            } else {
                write!(f, " [page index {page_index}")?;
                wrote_context = true;
            }
        }
        if let Some(object_id) = self.context.object_id {
            if wrote_context {
                write!(
                    f,
                    ", object {} {} R",
                    object_id.number, object_id.generation
                )?;
            } else {
                write!(
                    f,
                    " [object {} {} R",
                    object_id.number, object_id.generation
                )?;
                wrote_context = true;
            }
        }
        if wrote_context {
            f.write_str("]")?;
        }
        Ok(())
    }
}

impl fmt::Debug for PdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PdfError")
            .field("kind", &self.kind)
            .field("context", &self.context)
            .field("resource_limit", &self.resource_limit)
            .field("has_source", &self.source.is_some())
            .finish()
    }
}

impl std::error::Error for PdfError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn std::error::Error + 'static))
    }
}

impl From<std::io::Error> for PdfError {
    fn from(err: std::io::Error) -> Self {
        Self::from_source(PdfErrorKind::Io, err)
    }
}

#[derive(Debug)]
struct MessageError(String);

impl fmt::Display for MessageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for MessageError {}

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
    /// Zero-based page index where the warning occurred, if available/applicable.
    pub page: Option<usize>,
    /// Element context (e.g., "char at offset 42"), if available/applicable.
    pub element: Option<String>,
    /// Zero-based content-stream operator index, if available/applicable.
    pub operator_index: Option<usize>,
    /// Font name associated with the warning, if available/applicable.
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
        let mut error = PdfError::other(self.to_string()).during("strict warning escalation");
        if let Some(page) = self.page {
            error = error.at_page(page);
        }
        error
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExtractOptions {
    /// Maximum recursion depth for nested Form XObjects (default: 10).
    pub max_recursion_depth: usize,
    /// Maximum number of objects extracted per page (default: 100,000).
    pub max_objects_per_page: usize,
    /// Maximum content stream bytes to process (default: 100 MB).
    pub max_stream_bytes: usize,
    /// Whether to collect warnings during extraction (default: true).
    pub collect_warnings: bool,
    /// Unicode normalization to apply to extracted character text
    /// (default: [`UnicodeNorm::None`]).
    ///
    /// A font's ToUnicode mapping is what the document says its glyphs mean,
    /// and extraction reports it unchanged, as Python pdfplumber does. Even
    /// canonical normalization rewrites code points a caller may be looking
    /// for — U+037E, the Greek question mark, becomes an ordinary semicolon.
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
    /// Character deduplication options (default: `None`, no deduplication).
    ///
    /// Some PDF generators draw a glyph several times a fraction of a point
    /// apart to fake bold, and each draw is a real character on the page.
    /// Extraction reports them all, as Python pdfplumber does; set this to
    /// remove them during extraction, or call `Page::dedupe_chars` afterwards.
    pub dedupe: Option<crate::dedupe::DedupeOptions>,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            max_recursion_depth: 10,
            max_objects_per_page: 100_000,
            max_stream_bytes: 100 * 1024 * 1024,
            collect_warnings: true,
            unicode_norm: UnicodeNorm::None,
            extract_image_data: false,
            strict_mode: false,
            max_input_bytes: None,
            max_pages: None,
            max_total_image_bytes: None,
            max_total_objects: None,
            dedupe: None,
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
    use std::error::Error as _;

    // --- PdfError tests ---

    #[test]
    fn pdf_error_parse_error_creation() {
        let err = PdfError::parse("invalid xref");
        assert_eq!(err.kind(), PdfErrorKind::Parse);
        assert!(err.to_string().contains("valid, supported PDF"));
        assert_eq!(err.source().unwrap().to_string(), "invalid xref");
        assert!(!err.to_string().contains("invalid xref"));
    }

    #[test]
    fn pdf_error_io_error_creation() {
        let source = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = PdfError::from(source);
        assert_eq!(err.kind(), PdfErrorKind::Io);
        assert_eq!(
            err.source()
                .unwrap()
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .kind(),
            std::io::ErrorKind::NotFound
        );
        assert!(!err.to_string().contains("file not found"));
    }

    #[test]
    fn pdf_error_font_error_creation() {
        let err = PdfError::font("missing glyph widths");
        assert_eq!(err.kind(), PdfErrorKind::Font);
        assert!(err.to_string().contains("font data"));
    }

    #[test]
    fn pdf_error_interpreter_error_creation() {
        let err = PdfError::interpreter("unknown operator");
        assert_eq!(err.kind(), PdfErrorKind::Interpreter);
        assert!(err.to_string().contains("page content"));
    }

    #[test]
    fn pdf_error_resource_limit_exceeded() {
        let err = PdfError::limit_exceeded("max_input_bytes", 1024, 2048);
        assert_eq!(
            err.to_string(),
            "resource limit max_input_bytes exceeded (limit 1024, observed 2048); raise the configured limit or use a smaller input"
        );
    }

    #[test]
    fn pdf_error_resource_limit_exceeded_structured_fields() {
        let err = PdfError::limit_exceeded("max_pages", 10, 25);
        let limit = err.resource_limit().unwrap();
        assert_eq!(limit.name, "max_pages");
        assert_eq!(limit.limit, 10);
        assert_eq!(limit.observed, 25);
    }

    #[test]
    fn pdf_error_password_required() {
        let err = PdfError::password_required();
        assert_eq!(err.kind(), PdfErrorKind::PasswordRequired);
        assert!(err.to_string().contains("password-aware open methods"));
    }

    #[test]
    fn pdf_error_invalid_password() {
        let err = PdfError::invalid_password();
        assert_eq!(err.kind(), PdfErrorKind::InvalidPassword);
        assert!(err.to_string().contains("verify the supplied password"));
    }

    #[test]
    fn pdf_error_other() {
        let err = PdfError::other("something went wrong");
        assert_eq!(err.kind(), PdfErrorKind::Other);
        assert!(!err.to_string().contains("something went wrong"));
        assert_eq!(err.source().unwrap().to_string(), "something went wrong");
    }

    #[test]
    fn pdf_error_implements_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(PdfError::parse("test"));
        assert!(err.to_string().contains("valid, supported PDF"));
    }

    #[test]
    fn pdf_error_context_builders_are_structured() {
        let err = PdfError::parse("test")
            .during("load page")
            .at_page(4)
            .at_object(12, 2);
        assert_eq!(err.context().operation, Some("load page"));
        assert_eq!(err.context().page_index, Some(4));
        assert_eq!(
            err.context().object_id,
            Some(PdfObjectId {
                number: 12,
                generation: 2
            })
        );
    }

    #[test]
    fn pdf_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing file");
        let pdf_err: PdfError = io_err.into();
        assert_eq!(pdf_err.kind(), PdfErrorKind::Io);
        assert!(!pdf_err.to_string().contains("missing file"));
        assert!(!format!("{pdf_err:?}").contains("missing file"));
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
        assert_eq!(opts.unicode_norm, UnicodeNorm::None);
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
            dedupe: None,
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
        assert_eq!(err.kind(), PdfErrorKind::Other);
        assert_eq!(err.context().page_index, Some(0));
        assert!(!err.to_string().contains("font not found"));
        assert!(err.source().unwrap().to_string().contains("font not found"));
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
    fn resource_limit_details_are_cloneable_and_comparable() {
        let err = PdfError::limit_exceeded("max_input_bytes", 100, 200);
        let details = err.resource_limit().unwrap().clone();
        assert_eq!(details, *err.resource_limit().unwrap());
    }
}
