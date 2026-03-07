//! Internal PDF/UA-1 rule checker functions.
//!
//! These functions implement the per-element and per-page checks that the
//! [`A11yAnalyzer`](crate::A11yAnalyzer) dispatches to. They are `pub(crate)`
//! — not part of the public API.

use pdfplumber_core::StructElement;

use crate::rules::{Severity, Violation};

// ─────────────────────────────────────────────────────────────────────────────
// Role table
// ─────────────────────────────────────────────────────────────────────────────

/// Standard PDF/UA structure element role names.
pub(crate) const STANDARD_ROLES: &[&str] = &[
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

pub(crate) fn is_standard_role(role: &str) -> bool {
    STANDARD_ROLES.contains(&role)
}

// ─────────────────────────────────────────────────────────────────────────────
// Structure tree checkers
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn check_structure_tree(
    elements: &[&StructElement],
    violations: &mut Vec<Violation>,
    emit_info: bool,
) {
    let mut heading_stack: Vec<u8> = Vec::new(); // UA-006: heading nesting

    for elem in elements {
        check_element(elem, violations, &mut heading_stack, emit_info);
    }
}

pub(crate) fn check_element(
    elem: &StructElement,
    violations: &mut Vec<Violation>,
    heading_stack: &mut Vec<u8>,
    emit_info: bool,
) {
    let role = &elem.element_type;

    // UA-002: Standard role names
    if !is_standard_role(role) && !role.starts_with('/') {
        violations.push(Violation::new(
            "UA-002",
            Severity::Warning,
            format!("Non-standard structure type '{role}' — ensure it has a RoleMap entry"),
        ));
    } else if emit_info && !role.starts_with('/') {
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

// ─────────────────────────────────────────────────────────────────────────────
// Page-level checkers
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn check_page_structure(
    page: &pdfplumber::Page,
    page_idx: usize,
    elements: &[&StructElement],
    violations: &mut Vec<Violation>,
) {
    // UA-007: All content tagged
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
