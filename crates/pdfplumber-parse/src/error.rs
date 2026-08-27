//! Error types for the parsing and interpreter layers.
//!
//! Uses [`thiserror`] for ergonomic error derivation. Provides [`BackendError`]
//! that wraps backend-specific errors and converts them to [`PdfError`].

use pdfplumber_core::{PdfError, PdfErrorKind};
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
        if let BackendError::Core(error) = err {
            return error;
        }

        let kind = match &err {
            BackendError::Parse(_) => PdfErrorKind::Parse,
            BackendError::Io(_) => PdfErrorKind::Io,
            BackendError::Font(_) => PdfErrorKind::Font,
            BackendError::Interpreter(_) => PdfErrorKind::Interpreter,
            BackendError::Core(_) => unreachable!("core errors return above"),
        };
        PdfError::from_source(kind, err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;

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
        let pdf_err = PdfError::font("bad metrics");
        let err: BackendError = pdf_err.into();
        assert!(matches!(err, BackendError::Core(_)));
    }

    #[test]
    fn backend_error_to_pdf_error_parse() {
        let backend = BackendError::Parse("bad syntax".to_string());
        let pdf_err: PdfError = backend.into();
        assert_eq!(pdf_err.kind(), PdfErrorKind::Parse);
        assert_eq!(
            pdf_err.source().unwrap().to_string(),
            "PDF parse error: bad syntax"
        );
    }

    #[test]
    fn backend_error_to_pdf_error_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let backend = BackendError::Io(io_err);
        let pdf_err: PdfError = backend.into();
        assert_eq!(pdf_err.kind(), PdfErrorKind::Io);
        assert!(!pdf_err.to_string().contains("denied"));
        let backend = pdf_err
            .source()
            .unwrap()
            .downcast_ref::<BackendError>()
            .unwrap();
        assert_eq!(backend.source().unwrap().to_string(), "denied");
    }

    #[test]
    fn backend_error_to_pdf_error_font() {
        let backend = BackendError::Font("missing widths".to_string());
        let pdf_err: PdfError = backend.into();
        assert_eq!(pdf_err.kind(), PdfErrorKind::Font);
        assert!(
            pdf_err
                .source()
                .unwrap()
                .to_string()
                .contains("missing widths")
        );
    }

    #[test]
    fn backend_error_to_pdf_error_interpreter() {
        let backend = BackendError::Interpreter("stack underflow".to_string());
        let pdf_err: PdfError = backend.into();
        assert_eq!(pdf_err.kind(), PdfErrorKind::Interpreter);
        assert!(
            pdf_err
                .source()
                .unwrap()
                .to_string()
                .contains("stack underflow")
        );
    }

    #[test]
    fn backend_error_to_pdf_error_core_passthrough() {
        let original = PdfError::limit_exceeded("max_input_bytes", 1024, 2048);
        let backend = BackendError::Core(original);
        let pdf_err: PdfError = backend.into();
        assert_eq!(pdf_err.kind(), PdfErrorKind::ResourceLimit);
        assert_eq!(pdf_err.resource_limit().unwrap().observed, 2048);
    }

    #[test]
    fn backend_error_implements_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(BackendError::Parse("test".to_string()));
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn backend_error_core_password_required_passthrough() {
        let backend = BackendError::Core(PdfError::password_required());
        let pdf_err: PdfError = backend.into();
        assert_eq!(pdf_err.kind(), PdfErrorKind::PasswordRequired);
    }

    #[test]
    fn backend_error_core_invalid_password_passthrough() {
        let backend = BackendError::Core(PdfError::invalid_password());
        let pdf_err: PdfError = backend.into();
        assert_eq!(pdf_err.kind(), PdfErrorKind::InvalidPassword);
    }
}
