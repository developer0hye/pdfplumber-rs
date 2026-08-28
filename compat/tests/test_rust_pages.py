"""Contracts for ergonomic, lazy Rust page access (DX-004)."""

from __future__ import annotations

import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
PDF_SOURCE = (REPO_ROOT / "crates/pdfplumber/src/pdf.rs").read_text(encoding="utf-8")
CRATE_DOCS = (REPO_ROOT / "crates/pdfplumber/src/lib.rs").read_text(encoding="utf-8")
README = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
ROADMAP = (REPO_ROOT / "ROADMAP.md").read_text(encoding="utf-8")
CHANGELOG = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
PDFIUM_REFERENCE = REPO_ROOT / "references" / "pdfium-render.md"


class RustPageApiContractTests(unittest.TestCase):
    def test_pages_is_a_borrowed_collection_view(self) -> None:
        self.assertRegex(
            PDF_SOURCE,
            r"pub struct Pages<'a>\s*\{\s*pdf: &'a Pdf,\s*\}",
        )
        self.assertIn("pub fn pages(&self) -> Pages<'_>", PDF_SOURCE)
        self.assertIn("pub fn len(&self) -> usize", PDF_SOURCE)
        self.assertIn("pub fn is_empty(&self) -> bool", PDF_SOURCE)
        self.assertRegex(
            PDF_SOURCE,
            r"pub fn get\(&self, index: usize\) -> Result<Page, PdfError>",
        )

    def test_pages_supports_standard_lazy_iteration(self) -> None:
        self.assertIn("pub fn iter(&self) -> PagesIter<'a>", PDF_SOURCE)
        self.assertIn("impl<'a> IntoIterator for Pages<'a>", PDF_SOURCE)
        self.assertIn("impl DoubleEndedIterator for PagesIter<'_>", PDF_SOURCE)
        self.assertIn("impl std::iter::FusedIterator for PagesIter<'_>", PDF_SOURCE)

    def test_primary_docs_show_selection_and_iteration(self) -> None:
        for document in (README, CRATE_DOCS):
            normalized = " ".join(
                line.removeprefix("//!").strip() for line in document.splitlines()
            )
            self.assertIn("pdf.pages().get(0)", normalized)
            self.assertIn("for page in pdf.pages()", normalized)
            self.assertIn("on demand", normalized)
            self.assertIn("does not clone", normalized)

    def test_roadmap_advances_after_page_traversal(self) -> None:
        self.assertNotIn("### Make page traversal explicit", ROADMAP)
        self.assertIn("DIST-001", ROADMAP)
        normalized = " ".join(ROADMAP.split())
        self.assertRegex(normalized, r"(?i)clean Rust package consumption")

    def test_public_change_and_design_sources_are_traceable(self) -> None:
        self.assertIn("`Pdf::pages`", CHANGELOG)
        self.assertIn("`Pages::get`", CHANGELOG)
        self.assertTrue(PDFIUM_REFERENCE.is_file())
        reference = PDFIUM_REFERENCE.read_text(encoding="utf-8")
        self.assertIn("pdfium-render 0.9.3", reference)
        self.assertIn("PdfPages::get", reference)
        self.assertIn("PdfPages::iter", reference)
        self.assertIn("std::iter::IntoIterator", reference)


if __name__ == "__main__":
    unittest.main()
