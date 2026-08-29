"""Contracts for Python page numbers and Rust page indexes (DOC-005)."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GUIDE_PATH = REPO_ROOT / "docs" / "page-numbering.md"


def compact(text: str) -> str:
    return " ".join(text.split())


class PageNumberingDocumentationContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.guide = (
            GUIDE_PATH.read_text(encoding="utf-8") if GUIDE_PATH.is_file() else ""
        )
        cls.compact_guide = compact(cls.guide)

    def test_canonical_guide_is_linked_from_every_relevant_surface(self) -> None:
        self.assertTrue(GUIDE_PATH.is_file(), f"missing guide: {GUIDE_PATH}")

        links = {
            "README.md": "[page-numbering guide](docs/page-numbering.md)",
            "crates/pdfplumber-py/README.md": (
                "[page-numbering guide](../../docs/page-numbering.md)"
            ),
            "crates/pdfplumber-wasm/README.md": (
                "[page-numbering guide](../../docs/page-numbering.md)"
            ),
            "docs/python-migration.md": ("[page-numbering guide](page-numbering.md)"),
            "docs/pre-parity-python-migration.md": (
                "[page-numbering guide](page-numbering.md)"
            ),
            "docs/rust-api.md": "[page-numbering guide](page-numbering.md)",
        }
        for relative, link in links.items():
            with self.subTest(document=relative):
                rendered = (REPO_ROOT / relative).read_text(encoding="utf-8")
                self.assertIn(link, rendered)

        changelog = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        self.assertRegex(
            changelog,
            r"(?im)^- \*\*Page numbering:\*\* .*page-numbering guide",
        )

    def test_python_positions_numbers_and_selection_are_distinct(self) -> None:
        for statement in (
            "`pdf.pages` is a Python list, so its positions are zero-based",
            "`pdf.pages[0].page_number == 1`",
            "`pages=` accepts one-based document page numbers",
            "pdfplumber.open(path, pages=(3, 5))",
            "[page.page_number for page in pdf.pages] == [3, 5]",
            "selection is deduplicated and returned in document order",
            "an empty selection produces an empty page list",
            "selected pages preserve their original document page numbers",
            "derived pages copy the immediate parent's current `page_number`",
            "object dictionaries and serialized Python output use the same one-based `page_number`",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_rust_wasm_and_extension_indexes_remain_zero_based(self) -> None:
        for snippet in (
            "let first = pdf.pages().get(0)?;",
            "assert_eq!(first.page_number(), 0);",
            "pdf.page(index)",
            ".enumerate()",
            "assert_eq!(index, page.page_number());",
            "const first = pdf.page(0);",
            "first.pageNumber === 0",
        ):
            with self.subTest(snippet=snippet):
                self.assertIn(snippet, self.guide)

        for statement in (
            "Rust `Page::page_number()` is a zero-based index despite its historical name",
            "WebAssembly follows the Rust convention",
            "`document.rust.bookmarks()` destinations are zero-based",
            "`document.rust.form_fields()` page indexes are zero-based",
            "`document.rust.extract_images(page_index)` accepts a zero-based index",
            "the compatibility facade and the Rust-only namespace use different bases",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_conversions_validate_the_boundary_and_happen_once(self) -> None:
        for conversion in (
            "python_page_number = rust_page_index + 1",
            "rust_page_index = python_page_number - 1",
            "python_page_number >= 1",
            "rust_page_index < pdf.pages().len()",
        ):
            with self.subTest(conversion=conversion):
                self.assertIn(conversion, self.guide)

        for rule in (
            "validate before subtracting",
            "convert exactly once at the surface boundary",
            "do not apply a blanket increment or decrement",
            "PDF page labels are separate metadata",
            "an out-of-range Rust index returns `PdfError`",
            "an out-of-range WebAssembly index throws a JavaScript error",
        ):
            with self.subTest(rule=rule):
                self.assertIn(rule, self.compact_guide)

    def test_persistence_and_claims_keep_the_index_base_explicit(self) -> None:
        for field_name in (
            "page_number_one_based",
            "page_index_zero_based",
            "bookmark_page_index_zero_based",
        ):
            with self.subTest(field_name=field_name):
                self.assertIn(field_name, self.guide)

        for rule in (
            "Do not persist or log a cross-surface field named only `page`",
            "record the base in the field name or schema",
            "changing an index base is a data migration",
            "page-number documentation is not compatibility evidence",
            "does not approve a compatibility deviation",
        ):
            with self.subTest(rule=rule):
                self.assertIn(rule, self.compact_guide)

        self.assertNotRegex(self.guide, re.compile(r"\b\d+(?:\.\d+)?%\b"))


if __name__ == "__main__":
    unittest.main()
