"""Contracts for actionable, source-preserving Rust errors (DX-007)."""

from __future__ import annotations

import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
ERROR_SOURCE = (REPO_ROOT / "crates/pdfplumber-core/src/error.rs").read_text(
    encoding="utf-8"
)
PDF_SOURCE = (REPO_ROOT / "crates/pdfplumber/src/pdf.rs").read_text(encoding="utf-8")
README = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
CRATE_DOCS = (REPO_ROOT / "crates/pdfplumber/src/lib.rs").read_text(encoding="utf-8")
ROADMAP = (REPO_ROOT / "ROADMAP.md").read_text(encoding="utf-8")
CHANGELOG = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
GUIDE = REPO_ROOT / "docs/rust-errors.md"
REFERENCE = REPO_ROOT / "references/rust-errors.md"


class RustErrorContractTests(unittest.TestCase):
    def test_public_error_surface_is_typed_and_opaque(self) -> None:
        self.assertIn("pub enum PdfErrorKind", ERROR_SOURCE)
        self.assertIn("pub struct PdfObjectId", ERROR_SOURCE)
        self.assertIn("pub struct PdfErrorContext", ERROR_SOURCE)
        self.assertIn("pub struct PdfResourceLimit", ERROR_SOURCE)
        self.assertRegex(ERROR_SOURCE, r"pub struct PdfError\s*\{")
        self.assertNotRegex(ERROR_SOURCE, r"pub enum PdfError\s*\{")
        for accessor in ("kind", "context", "resource_limit"):
            self.assertRegex(ERROR_SOURCE, rf"pub (?:const )?fn {accessor}\(")

    def test_error_sources_are_preserved_but_default_formatting_is_safe(self) -> None:
        self.assertRegex(
            ERROR_SOURCE,
            r"source:\s*Option<Box<dyn std::error::Error \+ Send \+ Sync \+ 'static>>",
        )
        self.assertRegex(
            ERROR_SOURCE,
            r"fn source\(&self\)\s*->\s*Option<&\(dyn std::error::Error \+ 'static\)>",
        )
        self.assertIn("impl fmt::Debug for PdfError", ERROR_SOURCE)
        self.assertIn("has_source", ERROR_SOURCE)
        self.assertNotIn("field(\"source\", &self.source)", ERROR_SOURCE)

    def test_page_and_object_context_is_attached_at_the_facade_boundary(self) -> None:
        self.assertIn(".at_page(", PDF_SOURCE)
        self.assertIn(".at_object(", PDF_SOURCE)
        for operation in ("load page", "read page media box", "interpret page content"):
            self.assertIn(f'"{operation}"', PDF_SOURCE)

    def test_primary_docs_explain_diagnostics_and_roadmap_advances(self) -> None:
        self.assertTrue(GUIDE.is_file())
        guide = GUIDE.read_text(encoding="utf-8")
        normalized = " ".join(guide.split())
        self.assertRegex(normalized, r"(?i)Error::source.*underlying cause")
        self.assertRegex(normalized, r"(?i)Display.*Debug.*document content")
        self.assertRegex(normalized, r"(?i)page index.*object ID.*zero-based")
        self.assertRegex(normalized, r"(?i)source chain.*opt-in.*sensitive")
        for document in (README, CRATE_DOCS):
            self.assertIn("rust-errors", document)
        self.assertIn("`PdfErrorKind`", CHANGELOG)
        self.assertNotIn("### Make Rust failures actionable", ROADMAP)
        self.assertIn("DX-017", ROADMAP)
        self.assertTrue(REFERENCE.is_file())
        reference = REFERENCE.read_text(encoding="utf-8")
        self.assertIn("std::error::Error::source", reference)
        self.assertRegex(reference, r"thiserror.*2\.0\.20")


if __name__ == "__main__":
    unittest.main()
