//! Error types for the parsing and interpreter layers.
//!
//! Uses [`thiserror`] for ergonomic error derivation. Provides [`BackendError`]
//! that wraps backend-specific errors and converts them to [`PdfError`].

use pdfplumber_core::PdfError;
use thiserror::Error;

/// Error type for PDF parsing backend operations.
///
/// Wraps backend-specific errors and provides conversion to [`PdfError`]
/// for unified error handling across the library.
#[derive(Debug, Error)]
pub enum BackendError {
    /// Error from PDF parsing (structure, syntax, object resolution).
    #[error("PDF parse error: {0}")]
    Parse(String),

    /// Error reading PDF data.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Error resolving font or encoding information.
    #[error("font error: {0}")]
    Font(String),

    /// Error during content stream interpretation.
    #[error("interpreter error: {0}")]
    Interpreter(String),

    /// A core library error.
    #[error(transparent)]
    Core(#[from] PdfError),
}

impl From<BackendError> for PdfError {
    fn from(err: BackendError) -> Self {
        match err {
            BackendError::Parse(msg) => PdfError::ParseError(msg),
            BackendError::Io(e) => PdfError::IoError(e.to_string()),
            BackendError::Font(msg) => PdfError::FontError(msg),
            BackendError::Interpreter(msg) => PdfError::InterpreterError(msg),
            BackendError::Core(e) => e,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_error_parse() {
        let err = BackendError::Parse("invalid xref table".to_string());
        assert_eq!(err.to_string(), "PDF parse error: invalid xref table");
    }

    #[test]
    fn backend_error_io_from_std() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err: BackendError = io_err.into();
        assert!(matches!(err, BackendError::Io(_)));
        assert!(err.to_string().contains("file missing"));
    }

    #[test]
    fn backend_error_from_pdf_error() {
        let pdf_err = PdfError::FontError("bad metrics".to_string());
        let err: BackendError = pdf_err.into();
        assert!(matches!(err, BackendError::Core(_)));
    }

    #[test]
    fn backend_error_to_pdf_error_parse() {
        let backend = BackendError::Parse("bad syntax".to_string());
        let pdf_err: PdfError = backend.into();
        assert_eq!(pdf_err, PdfError::ParseError("bad syntax".to_string()));
    }

    #[test]
    fn backend_error_to_pdf_error_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let backend = BackendError::Io(io_err);
        let pdf_err: PdfError = backend.into();
        assert!(matches!(pdf_err, PdfError::IoError(_)));
        assert!(pdf_err.to_string().contains("denied"));
    }

    #[test]
    fn backend_error_to_pdf_error_font() {
        let backend = BackendError::Font("missing widths".to_string());
        let pdf_err: PdfError = backend.into();
        assert_eq!(pdf_err, PdfError::FontError("missing widths".to_string()));
    }

    #[test]
    fn backend_error_to_pdf_error_interpreter() {
        let backend = BackendError::Interpreter("stack underflow".to_string());
        let pdf_err: PdfError = backend.into();
        assert_eq!(
            pdf_err,
            PdfError::InterpreterError("stack underflow".to_string())
        );
    }

    #[test]
    fn backend_error_to_pdf_error_core_passthrough() {
        let original = PdfError::ResourceLimitExceeded {
            limit_name: "max_input_bytes".to_string(),
            limit_value: 1024,
            actual_value: 2048,
        };
        let backend = BackendError::Core(original.clone());
        let pdf_err: PdfError = backend.into();
        assert_eq!(pdf_err, original);
    }

    #[test]
    fn backend_error_implements_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(BackendError::Parse("test".to_string()));
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn backend_error_core_password_required_passthrough() {
        let backend = BackendError::Core(PdfError::PasswordRequired);
        let pdf_err: PdfError = backend.into();
        assert_eq!(pdf_err, PdfError::PasswordRequired);
    }

    #[test]
    fn backend_error_core_invalid_password_passthrough() {
        let backend = BackendError::Core(PdfError::InvalidPassword);
        let pdf_err: PdfError = backend.into();
        assert_eq!(pdf_err, PdfError::InvalidPassword);
    }
}
