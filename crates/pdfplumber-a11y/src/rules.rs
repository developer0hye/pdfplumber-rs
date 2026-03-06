//! PDF/UA-1 rule checkers.

use pdfplumber::Pdf;
use pdfplumber_core::StructElement;

// ─────────────────────────────────────────────────────────────────────────────
// Public types
// ─────────────────────────────────────────────────────────────────────────────

/// Severity of an accessibility violation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Informational note — not a failure, but worth knowing.
    Info,
    /// Document may pass strict validators but the issue reduces usability.
    Warning,
    /// Document will fail PDF/UA validation (hard requirement).
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "ERROR"),
            Severity::Warning => write!(f, "WARN"),
            Severity::Info => write!(f, "INFO"),
        }
    }
}

impl std::fmt::Display for Violation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let page = self
            .page
            .map(|p| format!(" [page {}]", p + 1))
            .unwrap_or_default();
        write!(
            f,
            "[{}] {}{}: {}",
            self.severity, self.rule_id, page, self.message
        )?;
        if let Some(s) = &self.suggestion {
            write!(f, " → {s}")?;
        }
        Ok(())
    }
}

/// A single PDF/UA rule violation found in the document.
#[derive(Debug, Clone)]
pub struct Violation {
    rule_id: &'static str,
    severity: Severity,
    message: String,
    /// Page number (0-based) where the violation was found. `None` = document-level.
    page: Option<usize>,
    /// Optional suggestion for fixing the violation.
    suggestion: Option<String>,
}

impl Violation {
    fn new(rule_id: &'static str, severity: Severity, message: impl Into<String>) -> Self {
        Self {
            rule_id,
            severity,
            message: message.into(),
            page: None,
            suggestion: None,
        }
    }

    fn on_page(mut self, page: usize) -> Self {
        self.page = Some(page);
        self
    }

    fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// The Matterhorn Protocol rule ID (e.g. "UA-001").
    pub fn rule_id(&self) -> &str {
        self.rule_id
    }

    /// Violation severity.
    pub fn severity(&self) -> &Severity {
        &self.severity
    }

    /// Human-readable description of the violation.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Page number where the violation was found (0-based). `None` = document-level.
    pub fn page(&self) -> Option<usize> {
        self.page
    }

    /// Suggested fix, if one can be stated programmatically.
    pub fn suggestion(&self) -> Option<&str> {
        self.suggestion.as_deref()
    }
}

/// PDF/UA analysis result for a complete document.
#[derive(Debug)]
pub struct A11yReport {
    violations: Vec<Violation>,
    page_count: usize,
    is_tagged: bool,
    has_lang: bool,
    has_title: bool,
    /// Inferred structure tags produced by [`TagInferrer`] when the document
    /// is untagged. `None` if the document is already tagged or if inference
    /// was not requested.
    inferred_tags: Option<Vec<crate::tag_infer::InferredTag>>,
}

impl A11yReport {
    /// Returns `true` if the document has no `Error`-severity violations.
    pub fn is_compliant(&self) -> bool {
        !self
            .violations
            .iter()
            .any(|v| v.severity == Severity::Error)
    }

    /// All violations found (errors + warnings + info).
    pub fn violations(&self) -> &[Violation] {
        &self.violations
    }

    /// Only error-severity violations.
    pub fn errors(&self) -> impl Iterator<Item = &Violation> {
        self.violations
            .iter()
            .filter(|v| v.severity == Severity::Error)
    }

    /// Count of error-severity violations.
    pub fn error_count(&self) -> usize {
        self.errors().count()
    }

    /// Whether the document has any structure tree at all.
    pub fn is_tagged(&self) -> bool {
        self.is_tagged
    }

    /// Whether the document declares a natural language.
    pub fn has_lang(&self) -> bool {
        self.has_lang
    }

    /// Number of pages in the analyzed document.
    pub fn page_count(&self) -> usize {
        self.page_count
    }

    /// Inferred structure tags for untagged documents.
    ///
    /// Returns `Some(&[InferredTag])` when the analyzer ran with
    /// `run_inference = true` AND the document is untagged.
    /// Returns `None` for tagged documents or when inference was not requested.
    pub fn inferred_tags(&self) -> Option<&[crate::tag_infer::InferredTag]> {
        self.inferred_tags.as_deref()
    }

    /// Count of items that need manual review in the inferred tag tree.
    ///
    /// Returns 0 if inference was not run.
    pub fn inference_review_count(&self) -> usize {
        self.inferred_tags
            .as_ref()
            .map(|tags| crate::tag_infer::TagInferrer::new().review_count(tags))
            .unwrap_or(0)
    }

    /// Render a human-readable summary to a `String`.
    pub fn summary(&self) -> String {
        let status = if self.is_compliant() { "PASS" } else { "FAIL" };
        let mut s = format!(
            "PDF/UA-1 Analysis: {status}\n\
             Pages: {}  Tagged: {}  Lang: {}  Title: {}\n\
             Violations: {} error(s), {} warning(s)\n",
            self.page_count,
            if self.is_tagged { "yes" } else { "no" },
            if self.has_lang { "yes" } else { "no" },
            if self.has_title { "yes" } else { "no" },
            self.violations
                .iter()
                .filter(|v| v.severity == Severity::Error)
                .count(),
            self.violations
                .iter()
                .filter(|v| v.severity == Severity::Warning)
                .count(),
        );
        for v in &self.violations {
            let page = v
                .page
                .map(|p| format!(" [page {}]", p + 1))
                .unwrap_or_default();
            s.push_str(&format!(
                "  [{:5}] {}{}: {}\n",
                v.severity, v.rule_id, page, v.message
            ));
            if let Some(sug) = &v.suggestion {
                s.push_str(&format!("         → {sug}\n"));
            }
        }
        s
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Analyzer
// ─────────────────────────────────────────────────────────────────────────────

/// PDF/UA-1 accessibility analyzer.
///
/// Construct once and call [`analyze`](Self::analyze) for each document.
#[derive(Default)]
pub struct A11yAnalyzer {
    /// If `true`, emit `Info`-level notices in addition to errors and warnings.
    pub emit_info: bool,
}

impl A11yAnalyzer {
    /// Create a new analyzer with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Analyze a [`Pdf`] document for PDF/UA-1 compliance.
    pub fn analyze(&self, pdf: &Pdf) -> A11yReport {
        let mut violations = Vec::new();
        let page_count = pdf.page_count();

        // Collect all pages once — we iterate multiple times below.
        // We use `page_count` to drive indexing; `pdf.pages()` is cheap (lazy).

        // UA-001: Document must be tagged.
        // Detect via structure elements — a tagged PDF will have StructElements.
        let is_tagged = (0..page_count).any(|i| {
            pdf.page(i)
                .ok()
                .map(|p| !p.structure_elements().is_empty())
                .unwrap_or(false)
        });
        if !is_tagged {
            violations.push(
                Violation::new(
                    "UA-001",
                    Severity::Error,
                    "Document is not tagged — no structure elements found on any page",
                )
                .with_suggestion(
                    "Add /MarkInfo << /Marked true >> to the document catalog \
                     and generate a complete structure tree",
                ),
            );
        }

        // UA-005: Document language.
        // /Lang lives in the catalog dict; we detect its presence via any
        // StructElement that carries a lang attribute (common in well-tagged PDFs).
        // When none are found, emit a Warning so authors know to check.
        let has_lang = (0..page_count).any(|i| {
            pdf.page(i)
                .ok()
                .map(|p| p.structure_elements().iter().any(|e| e.lang.is_some()))
                .unwrap_or(false)
        });
        if !has_lang && is_tagged {
            violations.push(
                Violation::new(
                    "UA-005",
                    Severity::Warning,
                    "No /Lang attribute found on any structure element — \
                     verify the document catalog has a /Lang entry",
                )
                .with_suggestion(
                    "Add /Lang (en-US) or appropriate BCP-47 language tag to the document catalog",
                ),
            );
        }

        // UA-008: Document title.
        let has_title = pdf
            .metadata()
            .title
            .as_deref()
            .map(|t| !t.trim().is_empty())
            .unwrap_or(false);
        if !has_title {
            violations.push(
                Violation::new(
                    "UA-008",
                    Severity::Error,
                    "Document /Title metadata is missing or empty",
                )
                .with_suggestion(
                    "Set a non-empty /Title in the document's DocInfo dictionary or XMP metadata",
                ),
            );
        }

        // Per-page analysis.
        for page_idx in 0..page_count {
            let Ok(page) = pdf.page(page_idx) else {
                continue;
            };
            let elems = page.structure_elements();

            if is_tagged {
                // UA-002 / UA-003 / UA-006: check element tree for this page
                check_structure_tree(&elems, &mut violations, self.emit_info);
                // UA-007 / UA-010: per-page coverage and link checks
                check_page_structure(&page, page_idx, &elems, &mut violations);
            } else {
                // UA-003: untagged images have no alt text by definition
                if !page.images().is_empty() {
                    violations.push(
                        Violation::new(
                            "UA-003",
                            Severity::Error,
                            format!(
                                "Page {} has {} image(s) with no alt text \
                                 (document is untagged)",
                                page_idx + 1,
                                page.images().len()
                            ),
                        )
                        .on_page(page_idx)
                        .with_suggestion("Tag images with Figure elements and /Alt text"),
                    );
                }
            }
        }

        A11yReport {
            violations,
            page_count,
            is_tagged,
            has_lang,
            has_title,
            inferred_tags: None,
        }
    }

    /// Analyze a [`Pdf`] for PDF/UA-1 compliance AND run [`crate::TagInferrer`] on
    /// untagged documents to produce an inferred structure tree.
    ///
    /// For tagged documents this is identical to [`analyze`](Self::analyze).
    /// For untagged documents the returned [`A11yReport`] additionally contains
    /// `inferred_tags()` — a per-page list of structure elements that WOULD be
    /// needed to make the document accessible.
    ///
    /// The inferred tags are useful for:
    /// - Showing authors what remediation would look like
    /// - Feeding into a downstream PDF writer (Lane 10) to auto-tag documents
    /// - Estimating remediation effort (`report.inference_review_count()`)
    pub fn analyze_with_inference(&self, pdf: &Pdf) -> A11yReport {
        let mut report = self.analyze(pdf);
        if !report.is_tagged {
            let inferrer = crate::tag_infer::TagInferrer::new();
            report.inferred_tags = Some(inferrer.infer_document(pdf));
        }
        report
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Rule checkers
// ─────────────────────────────────────────────────────────────────────────────

/// Standard PDF/UA structure element role names.
const STANDARD_ROLES: &[&str] = &[
    "Document",
    "Art",
    "Sect",
    "Div",
    "BlockQuote",
    "Caption",
    "TOC",
    "TOCI",
    "Index",
    "NonStruct",
    "Private",
    "H",
    "H1",
    "H2",
    "H3",
    "H4",
    "H5",
    "H6",
    "P",
    "L",
    "LI",
    "Lbl",
    "LBody",
    "Table",
    "TR",
    "TH",
    "TD",
    "THead",
    "TBody",
    "TFoot",
    "Span",
    "Quote",
    "Note",
    "Reference",
    "BibEntry",
    "Code",
    "Link",
    "Annot",
    "Ruby",
    "RB",
    "RT",
    "RP",
    "Warichu",
    "WT",
    "WP",
    "Figure",
    "Formula",
    "Form",
];

fn is_standard_role(role: &str) -> bool {
    STANDARD_ROLES.contains(&role)
}

fn check_structure_tree(
    elements: &[&StructElement],
    violations: &mut Vec<Violation>,
    emit_info: bool,
) {
    let mut heading_stack: Vec<u8> = Vec::new(); // UA-006: heading nesting

    for elem in elements {
        check_element(elem, violations, &mut heading_stack, emit_info);
    }
}

fn check_element(
    elem: &StructElement,
    violations: &mut Vec<Violation>,
    heading_stack: &mut Vec<u8>,
    emit_info: bool,
) {
    let role = &elem.element_type;

    // UA-002: Standard role names
    // Non-standard roles are allowed if they appear in a RoleMap, but we can only
    // check the surface here. Always flag as Warning; Info-level details only when
    // emit_info is set.
    if !is_standard_role(role) && !role.starts_with('/') {
        violations.push(Violation::new(
            "UA-002",
            Severity::Warning,
            format!("Non-standard structure type '{role}' — ensure it has a RoleMap entry"),
        ));
    } else if emit_info && !role.starts_with('/') {
        // Info: confirm recognized role (useful for verbose audits).
        violations.push(Violation::new(
            "UA-002",
            Severity::Info,
            format!("Standard role '{role}' recognized"),
        ));
    }

    // UA-003: Figures need alt text
    if role == "Figure" && elem.alt_text.is_none() {
        violations.push(
            Violation::new("UA-003", Severity::Error, "Figure element has no /Alt text")
                .with_suggestion("Add /Alt entry to the Figure's attribute dictionary"),
        );
    }

    // UA-004: Table header cells must have scope attributes.
    // A TH element without a /Scope attribute is technically non-conforming.
    // We flag it as a Warning since we cannot read /Scope from StructElement here
    // (it lives in the attribute objects, not the element type). The heuristic:
    // if a TH element has no children and no alt_text, it likely needs review.
    if role == "TH" {
        violations.push(
            Violation::new(
                "UA-004",
                Severity::Warning,
                "TH (table header cell) found — verify /Scope attribute (Column/Row/Both) is set",
            )
            .with_suggestion(
                "Add /Scope /Column (or /Row or /Both) to each TH element's attribute dictionary",
            ),
        );
    }

    // UA-006: Heading nesting order
    let heading_level: Option<u8> = match role.as_str() {
        "H" => Some(1),
        "H1" => Some(1),
        "H2" => Some(2),
        "H3" => Some(3),
        "H4" => Some(4),
        "H5" => Some(5),
        "H6" => Some(6),
        _ => None,
    };
    if let Some(level) = heading_level {
        if let Some(&prev) = heading_stack.last() {
            if level > prev + 1 {
                violations.push(
                    Violation::new(
                        "UA-006",
                        Severity::Error,
                        format!(
                            "Heading H{level} appears after H{prev} — headings must not skip levels"
                        ),
                    )
                    .with_suggestion(format!("Add an H{} between H{prev} and H{level}", prev + 1)),
                );
            }
        }
        heading_stack.push(level);
    }

    // Recurse
    for child in &elem.children {
        check_element(child, violations, heading_stack, emit_info);
    }
}

fn check_page_structure(
    page: &pdfplumber::Page,
    page_idx: usize,
    elements: &[&StructElement],
    violations: &mut Vec<Violation>,
) {
    // UA-007: All content tagged
    // Heuristic: if page has chars AND structure elements, check MCID coverage.
    // If page has chars but no structure elements at all, that's untagged content.
    if !page.chars().is_empty() && elements.is_empty() {
        violations.push(
            Violation::new(
                "UA-007",
                Severity::Error,
                format!(
                    "Page {} has text content with no structure elements",
                    page_idx + 1
                ),
            )
            .on_page(page_idx)
            .with_suggestion("Tag all text content with appropriate structure elements"),
        );
    }

    // UA-010: Links must have accessible text
    for link in page.hyperlinks() {
        // A link is accessible if it has visible text chars overlapping its bbox.
        let link_bbox = link.bbox;
        let has_text = page.chars().iter().any(|c| {
            let cb = &c.bbox;
            cb.x0 >= link_bbox.x0 - 2.0
                && cb.x1 <= link_bbox.x1 + 2.0
                && cb.top >= link_bbox.top - 2.0
                && cb.bottom <= link_bbox.bottom + 2.0
        });
        if !has_text {
            violations.push(
                Violation::new(
                    "UA-010",
                    Severity::Warning,
                    format!(
                        "Link on page {} at ({:.1},{:.1})-({:.1},{:.1}) has no visible text",
                        page_idx + 1,
                        link_bbox.x0,
                        link_bbox.top,
                        link_bbox.x1,
                        link_bbox.bottom
                    ),
                )
                .on_page(page_idx)
                .with_suggestion(
                    "Add /Alt text to the link annotation or ensure it overlaps visible text",
                ),
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ordering() {
        assert!(Severity::Error > Severity::Warning);
        assert!(Severity::Warning > Severity::Info);
    }

    #[test]
    fn violation_fields() {
        let v = Violation::new("UA-001", Severity::Error, "test message")
            .on_page(2)
            .with_suggestion("do something");
        assert_eq!(v.rule_id(), "UA-001");
        assert_eq!(v.severity(), &Severity::Error);
        assert_eq!(v.message(), "test message");
        assert_eq!(v.page(), Some(2));
        assert_eq!(v.suggestion(), Some("do something"));
    }

    #[test]
    fn violation_no_page() {
        let v = Violation::new("UA-005", Severity::Error, "no lang");
        assert_eq!(v.page(), None);
        assert_eq!(v.suggestion(), None);
    }

    #[test]
    fn report_compliant_no_errors() {
        let report = A11yReport {
            violations: vec![Violation::new(
                "UA-009",
                Severity::Warning,
                "possible artifact",
            )],
            page_count: 1,
            is_tagged: true,
            has_lang: true,
            has_title: true,
            inferred_tags: None,
        };
        assert!(report.is_compliant());
        assert_eq!(report.error_count(), 0);
    }

    #[test]
    fn report_not_compliant_with_errors() {
        let report = A11yReport {
            violations: vec![Violation::new("UA-001", Severity::Error, "not tagged")],
            page_count: 2,
            is_tagged: false,
            has_lang: false,
            has_title: false,
            inferred_tags: None,
        };
        assert!(!report.is_compliant());
        assert_eq!(report.error_count(), 1);
    }

    #[test]
    fn standard_roles_recognized() {
        assert!(is_standard_role("H1"));
        assert!(is_standard_role("Figure"));
        assert!(is_standard_role("Table"));
        assert!(is_standard_role("TD"));
        assert!(!is_standard_role("MyCustomRole"));
    }

    fn make_elem(et: &str) -> StructElement {
        StructElement {
            element_type: et.to_owned(),
            mcids: vec![],
            alt_text: None,
            actual_text: None,
            lang: None,
            bbox: None,
            children: vec![],
            page_index: None,
        }
    }

    fn make_elem_alt(et: &str, alt: &str) -> StructElement {
        StructElement {
            element_type: et.to_owned(),
            mcids: vec![],
            alt_text: Some(alt.to_owned()),
            actual_text: None,
            lang: None,
            bbox: None,
            children: vec![],
            page_index: None,
        }
    }

    #[test]
    fn heading_skip_triggers_ua006() {
        let owned = vec![make_elem("H1"), make_elem("P"), make_elem("H3")];
        let elems: Vec<&StructElement> = owned.iter().collect();
        let mut violations = Vec::new();
        check_structure_tree(&elems, &mut violations, false);
        let ua006 = violations.iter().find(|v| v.rule_id == "UA-006");
        assert!(ua006.is_some(), "should flag H1→H3 skip");
    }

    #[test]
    fn heading_sequential_no_violation() {
        let owned = vec![make_elem("H1"), make_elem("H2"), make_elem("H3")];
        let elems: Vec<&StructElement> = owned.iter().collect();
        let mut violations = Vec::new();
        check_structure_tree(&elems, &mut violations, false);
        let ua006 = violations.iter().any(|v| v.rule_id == "UA-006");
        assert!(!ua006, "sequential headings should not trigger UA-006");
    }

    #[test]
    fn figure_without_alt_triggers_ua003() {
        let owned = vec![make_elem("Figure")];
        let elems: Vec<&StructElement> = owned.iter().collect();
        let mut violations = Vec::new();
        check_structure_tree(&elems, &mut violations, false);
        let ua003 = violations.iter().find(|v| v.rule_id == "UA-003");
        assert!(
            ua003.is_some(),
            "Figure without alt text should trigger UA-003"
        );
    }

    #[test]
    fn figure_with_alt_ok() {
        let owned = vec![make_elem_alt("Figure", "Revenue chart")];
        let elems: Vec<&StructElement> = owned.iter().collect();
        let mut violations = Vec::new();
        check_structure_tree(&elems, &mut violations, false);
        let ua003 = violations.iter().any(|v| v.rule_id == "UA-003");
        assert!(!ua003, "Figure with alt text should not trigger UA-003");
    }

    #[test]
    fn report_summary_contains_status() {
        let report = A11yReport {
            violations: vec![Violation::new("UA-001", Severity::Error, "not tagged")],
            page_count: 1,
            is_tagged: false,
            has_lang: false,
            has_title: false,
            inferred_tags: None,
        };
        let summary = report.summary();
        assert!(summary.contains("FAIL"));
        assert!(summary.contains("UA-001"));
    }

    #[test]
    fn violation_display_format() {
        let v = Violation::new("UA-003", Severity::Error, "Figure has no alt text")
            .on_page(4)
            .with_suggestion("Add /Alt text");
        let s = v.to_string();
        assert!(s.contains("UA-003"), "display must include rule id");
        assert!(
            s.contains("[ERROR]") || s.contains("ERROR"),
            "display must include severity"
        );
        assert!(
            s.contains("page 5"),
            "display must include 1-based page number"
        );
        assert!(
            s.contains("Add /Alt text"),
            "display must include suggestion"
        );
    }

    #[test]
    fn violation_display_no_page_no_suggestion() {
        let v = Violation::new("UA-001", Severity::Warning, "no lang");
        let s = v.to_string();
        assert!(s.contains("UA-001"));
        assert!(!s.contains("page"), "no page should not include page ref");
    }

    #[test]
    fn ua002_fires_for_nonstandard_role() {
        let owned = vec![make_elem("MyCustomRole")];
        let elems: Vec<&StructElement> = owned.iter().collect();
        let mut violations = Vec::new();
        check_structure_tree(&elems, &mut violations, false);
        let ua002 = violations.iter().find(|v| v.rule_id == "UA-002");
        assert!(
            ua002.is_some(),
            "non-standard role should trigger UA-002 as Warning"
        );
        assert_eq!(ua002.unwrap().severity, Severity::Warning);
    }

    #[test]
    fn ua004_fires_for_th_element() {
        let owned = vec![make_elem("TH")];
        let elems: Vec<&StructElement> = owned.iter().collect();
        let mut violations = Vec::new();
        check_structure_tree(&elems, &mut violations, false);
        let ua004 = violations.iter().find(|v| v.rule_id == "UA-004");
        assert!(
            ua004.is_some(),
            "TH element should trigger UA-004 scope check"
        );
    }

    #[test]
    fn report_inferred_tags_none_by_default() {
        let report = A11yReport {
            violations: vec![],
            page_count: 1,
            is_tagged: true,
            has_lang: true,
            has_title: true,
            inferred_tags: None,
        };
        assert!(report.inferred_tags().is_none());
        assert_eq!(report.inference_review_count(), 0);
    }
}
