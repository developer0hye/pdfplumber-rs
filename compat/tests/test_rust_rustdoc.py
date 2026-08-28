"""Contracts for the stable Rust facade's generated API documentation."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CRATE_ROOT = (ROOT / "crates/pdfplumber/src/lib.rs").read_text(encoding="utf-8")
PDF_SOURCE = (ROOT / "crates/pdfplumber/src/pdf.rs").read_text(encoding="utf-8")
PAGE_SOURCE = (ROOT / "crates/pdfplumber/src/page.rs").read_text(encoding="utf-8")
CROPPED_PAGE_SOURCE = (ROOT / "crates/pdfplumber/src/cropped_page.rs").read_text(
    encoding="utf-8"
)
CI = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
README = (ROOT / "README.md").read_text(encoding="utf-8")
CHANGELOG = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
ROADMAP = (ROOT / "ROADMAP.md").read_text(encoding="utf-8")
GUIDE = ROOT / "docs/rust-api.md"
REFERENCE = ROOT / "references/rust-rustdoc.md"


def rustdoc_for(source: str, function: str) -> str:
    match = re.search(
        rf"(?m)(?P<docs>(?:\s*///[^\n]*\n)+)\s*pub fn {re.escape(function)}\b",
        source,
    )
    if match is None:
        raise AssertionError(
            f"public function {function} has no contiguous rustdoc block"
        )
    return match.group("docs")


class RustRustdocContractTests(unittest.TestCase):
    def test_crate_denies_missing_docs_and_broken_links(self) -> None:
        self.assertIn("#![deny(missing_docs)]", CRATE_ROOT)
        self.assertIn("#![deny(rustdoc::broken_intra_doc_links)]", CRATE_ROOT)

    def test_ci_builds_and_lints_the_all_feature_facade(self) -> None:
        normalized = " ".join(CI.split())
        self.assertIn("RUSTDOCFLAGS: -D warnings", normalized)
        self.assertIn(
            "cargo doc -p pdfplumber --no-deps --all-features",
            normalized,
        )
        self.assertIn(
            "cargo clippy -p pdfplumber --all-features --no-deps -- -D warnings "
            "-D clippy::missing_errors_doc -D clippy::missing_panics_doc",
            normalized,
        )

    def test_fallible_and_panicking_items_document_their_contracts(self) -> None:
        self.assertIn("# Errors", rustdoc_for(PDF_SOURCE, "validate_metadata"))
        self.assertIn("# Panics", rustdoc_for(PAGE_SOURCE, "extract_table"))

    def test_search_rustdoc_has_no_table_summary_bleed(self) -> None:
        docs = rustdoc_for(PAGE_SOURCE, "search")
        self.assertIn("Search for a text pattern", docs)
        self.assertNotIn("largest table", docs)

    def test_high_level_docs_explain_observable_semantics(self) -> None:
        constructor_docs = rustdoc_for(PAGE_SOURCE, "new")
        for detail in ("zero-based", "points", "rotation", "characters"):
            with self.subTest(item="Page::new", detail=detail):
                self.assertIn(detail, constructor_docs)

        dimensions_docs = rustdoc_for(PDF_SOURCE, "page_dimensions")
        for detail in ("zero-based", "points", "`None`"):
            with self.subTest(item="Pdf::page_dimensions", detail=detail):
                self.assertIn(detail, dimensions_docs)

        cropped_words_docs = rustdoc_for(CROPPED_PAGE_SOURCE, "extract_words")
        self.assertIn("Page::extract_words", cropped_words_docs)
        cropped_crop_docs = rustdoc_for(CROPPED_PAGE_SOURCE, "crop")
        self.assertIn("current coordinate system", cropped_crop_docs)

    def test_public_boundary_and_sources_are_documented(self) -> None:
        self.assertTrue(GUIDE.is_file())
        guide = GUIDE.read_text(encoding="utf-8")
        for item in (
            "`Pdf`",
            "`Pages`",
            "`PagesIter`",
            "`Page`",
            "`PageObjectKind`",
            "`CroppedPage`",
            "`FilteredPage`",
            "`PdfError`",
            "`PdfErrorKind`",
            "`PdfErrorContext`",
            "`PdfObjectId`",
            "`PdfResourceLimit`",
            "`pdfplumber::models`",
        ):
            with self.subTest(item=item):
                self.assertIn(item, guide)
        self.assertIn("missing_docs", guide)
        self.assertIn("missing_errors_doc", guide)
        self.assertIn("missing_panics_doc", guide)
        for public_entry in (README, CRATE_ROOT, CHANGELOG):
            self.assertIn("rust-api.md", public_entry)
        self.assertNotIn("### Complete public Rust API documentation", ROADMAP)
        self.assertIn("SCORE-010", ROADMAP)
        self.assertTrue(REFERENCE.is_file())
        reference = REFERENCE.read_text(encoding="utf-8")
        self.assertIn("The `missing_docs` lint", reference)
        self.assertIn("rustdoc lints", reference)


if __name__ == "__main__":
    unittest.main()
