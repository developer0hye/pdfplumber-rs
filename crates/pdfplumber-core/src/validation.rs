//! PDF validation types for detecting specification violations.
//!
//! Provides [`ValidationIssue`] for reporting detected issues and
//! [`Severity`] for classifying their impact on extraction.

use std::fmt;

/// Severity of a validation issue.
///
/// Indicates whether a PDF specification violation is likely to cause
/// extraction failures or is merely a non-conformance that still allows
/// best-effort extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    /// Specification violation likely to cause extraction failure.
    Error,
    /// Non-conformance but data is likely still extractable.
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
        }
    }
}

/// A validation issue found in a PDF document.
///
/// Describes a specific PDF specification violation or non-conformance,
/// including its severity, an identifying code, a human-readable message,
/// and an optional location within the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    /// Severity of the issue.
    pub severity: Severity,
    /// Machine-readable issue code (e.g., "MISSING_TYPE", "BROKEN_REF").
    pub code: String,
    /// Human-readable description of the issue.
    pub message: String,
    /// Optional location within the PDF (e.g., "page 3", "object 5 0").
    pub location: Option<String>,
}

impl ValidationIssue {
    /// Create a new validation issue.
    pub fn new(severity: Severity, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            location: None,
        }
    }

    /// Create a new validation issue with a location.
    pub fn with_location(
        severity: Severity,
        code: impl Into<String>,
        message: impl Into<String>,
        location: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            location: Some(location.into()),
        }
    }

    /// Returns `true` if the issue is an error.
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    /// Returns `true` if the issue is a warning.
    pub fn is_warning(&self) -> bool {
        self.severity == Severity::Warning
    }
}

impl fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.severity, self.code, self.message)?;
        if let Some(ref loc) = self.location {
            write!(f, " (at {loc})")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_display() {
        assert_eq!(Severity::Error.to_string(), "error");
        assert_eq!(Severity::Warning.to_string(), "warning");
    }

    #[test]
    fn severity_clone_and_eq() {
        let s1 = Severity::Error;
        let s2 = s1.clone();
        assert_eq!(s1, s2);
        assert_ne!(Severity::Error, Severity::Warning);
    }

    #[test]
    fn validation_issue_new() {
        let issue =
            ValidationIssue::new(Severity::Error, "MISSING_TYPE", "catalog missing /Type key");
        assert_eq!(issue.severity, Severity::Error);
        assert_eq!(issue.code, "MISSING_TYPE");
        assert_eq!(issue.message, "catalog missing /Type key");
        assert!(issue.location.is_none());
        assert!(issue.is_error());
        assert!(!issue.is_warning());
    }

    #[test]
    fn validation_issue_with_location() {
        let issue = ValidationIssue::with_location(
            Severity::Warning,
            "MISSING_FONT",
            "font /F1 not found in resources",
            "page 2",
        );
        assert_eq!(issue.severity, Severity::Warning);
        assert_eq!(issue.code, "MISSING_FONT");
        assert_eq!(issue.message, "font /F1 not found in resources");
        assert_eq!(issue.location.as_deref(), Some("page 2"));
        assert!(!issue.is_error());
        assert!(issue.is_warning());
    }

    #[test]
    fn validation_issue_display_without_location() {
        let issue = ValidationIssue::new(Severity::Error, "BROKEN_REF", "object 5 0 not found");
        assert_eq!(
            issue.to_string(),
            "[error] BROKEN_REF: object 5 0 not found"
        );
    }

    #[test]
    fn validation_issue_display_with_location() {
        let issue = ValidationIssue::with_location(
            Severity::Warning,
            "MISSING_FONT",
            "font /F1 referenced but not defined",
            "page 3",
        );
        assert_eq!(
            issue.to_string(),
            "[warning] MISSING_FONT: font /F1 referenced but not defined (at page 3)"
        );
    }

    #[test]
    fn validation_issue_clone_and_eq() {
        let issue1 = ValidationIssue::new(Severity::Error, "TEST", "test message");
        let issue2 = issue1.clone();
        assert_eq!(issue1, issue2);
    }
}
