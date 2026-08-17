"""Pinned-upstream behavioral coverage for parity-report page accounting."""

import unittest
from pathlib import Path
from unittest import mock

from scripts import parity_report


REPO_ROOT = Path(__file__).resolve().parents[2]


class ParityReportPageTests(unittest.TestCase):
    def test_python_side_extracts_every_page(self) -> None:
        fixtures = {
            "tests/fixtures/downloaded/pdffill-demo.pdf": 7,
            "tests/fixtures/generated/rotated_pages.pdf": 4,
        }

        for relative_path, page_count in fixtures.items():
            with self.subTest(fixture=relative_path):
                result = parity_report.python_side(str(REPO_ROOT / relative_path))

                self.assertEqual(result["page_count"], page_count)
                self.assertEqual(len(result["pages"]), page_count)
                self.assertEqual(
                    [page["page_number"] for page in result["pages"]],
                    list(range(1, page_count + 1)),
                )

    def test_document_comparison_flags_a_page_count_mismatch(self) -> None:
        expected = {
            "page_count": 2,
            "pages": [self.empty_page(1), self.empty_page(2)],
        }
        actual = {"page_count": 1, "pages": [self.empty_page(1)]}

        result = parity_report.compare_documents(expected, actual)

        self.assertFalse(result["page_count_equal"])
        self.assertEqual(result["pages"][0]["status"], "compared")
        self.assertEqual(result["pages"][1], {"page_number": 2, "status": "missing_in_rust"})

    def test_fixture_ids_preserve_directories_for_duplicate_basenames(self) -> None:
        downloaded = REPO_ROOT / "tests/fixtures/downloaded/pdffill-demo.pdf"
        crate_fixture = (
            REPO_ROOT / "crates/pdfplumber/tests/fixtures/pdfs/pdffill-demo.pdf"
        )

        downloaded_id = parity_report.fixture_id(str(downloaded), str(REPO_ROOT))
        crate_id = parity_report.fixture_id(str(crate_fixture), str(REPO_ROOT))

        self.assertEqual(downloaded_id, "tests/fixtures/downloaded/pdffill-demo.pdf")
        self.assertEqual(
            crate_id,
            "crates/pdfplumber/tests/fixtures/pdfs/pdffill-demo.pdf",
        )
        self.assertNotEqual(downloaded_id, crate_id)

    def test_report_returns_nonzero_for_page_count_mismatch(self) -> None:
        expected = {
            "page_count": 2,
            "pages": [self.empty_page(1), self.empty_page(2)],
        }
        actual = {"page_count": 1, "pages": [self.empty_page(1)]}

        with (
            mock.patch.object(parity_report.environment, "verify_reference"),
            mock.patch.object(
                parity_report,
                "find_fixtures",
                return_value=["/fixtures/a.pdf"],
            ),
            mock.patch.object(parity_report, "python_side", return_value=expected),
            mock.patch.object(parity_report, "rust_side", return_value=actual),
            mock.patch("sys.argv", ["parity_report.py", "--fixtures", "/fixtures"]),
            mock.patch("builtins.print"),
        ):
            status = parity_report.main()

        self.assertEqual(status, 1)

    @staticmethod
    def empty_page(page_number: int) -> dict:
        return {
            "page_number": page_number,
            "chars": [],
            "words": [],
            "page_text": "",
            "layout_text": "",
            "simple_text": "",
            "text_lines": [],
            "search": [],
            "tables": [],
            "annotations": [],
            "hyperlinks": [],
            "structure_tree": [],
        }


if __name__ == "__main__":
    unittest.main()
