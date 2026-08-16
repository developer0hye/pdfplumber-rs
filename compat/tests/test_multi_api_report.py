"""Behavioral coverage for the per-API differential report."""

import unittest
from unittest import mock

from scripts import parity_report


REQUESTED_APIS = {
    "page_text",
    "layout_text",
    "simple_text",
    "text_lines",
    "words",
    "search",
    "tables",
    "annotations",
    "hyperlinks",
    "structure_tree",
}


class MultiApiReportTests(unittest.TestCase):
    def test_python_page_collects_every_requested_upstream_api(self) -> None:
        class Page:
            chars = [{"text": "A"}]
            annots = [{"object_type": "annot"}]
            hyperlinks = [{"uri": "https://example.com"}]
            structure_tree = [{"type": "Document"}]

            @staticmethod
            def extract_text(*, layout: bool = False) -> str:
                return "layout" if layout else "page"

            @staticmethod
            def extract_text_simple() -> str:
                return "simple"

            @staticmethod
            def extract_text_lines() -> list[dict]:
                return [{"text": "line", "chars": []}]

            @staticmethod
            def extract_words() -> list[dict]:
                return [{"text": "word"}]

            @staticmethod
            def search(pattern: str) -> list[dict]:
                if pattern != parity_report.SEARCH_PATTERN:
                    raise AssertionError(f"unexpected pattern: {pattern!r}")
                return [{"text": "match", "groups": (), "chars": []}]

            @staticmethod
            def extract_tables() -> list:
                return [[['cell']]]

        result = parity_report.python_page(Page(), 1)

        self.assertTrue(REQUESTED_APIS.issubset(result))
        self.assertEqual(result["page_text"], "page")
        self.assertEqual(result["layout_text"], "layout")
        self.assertEqual(result["simple_text"], "simple")
        self.assertEqual(result["text_lines"][0]["text"], "line")
        self.assertEqual(result["search"][0]["groups"], ())
        self.assertEqual(result["annotations"], Page.annots)
        self.assertEqual(result["hyperlinks"], Page.hyperlinks)
        self.assertEqual(result["structure_tree"], Page.structure_tree)

    def test_document_comparison_reports_every_requested_api(self) -> None:
        expected_page = {
            "page_number": 1,
            "chars": [],
            "page_text": "page",
            "layout_text": "layout",
            "simple_text": "simple",
            "text_lines": [],
            "words": [],
            "search": [],
            "tables": [],
            "annotations": [],
            "hyperlinks": [],
            "structure_tree": [],
        }
        actual_page = dict(expected_page)
        expected = {"page_count": 1, "pages": [expected_page]}
        actual = {"page_count": 1, "pages": [actual_page]}

        page = parity_report.compare_documents(expected, actual)["pages"][0]

        self.assertTrue(REQUESTED_APIS.issubset(page))
        for api in REQUESTED_APIS:
            self.assertTrue(page[api].get("equal", True), api)

    def test_unsupported_candidate_api_is_an_explicit_failure(self) -> None:
        result = parity_report.compare_api_value(
            "upstream value",
            parity_report.unsupported_api("TEXT-EXTRA-001"),
        )

        self.assertEqual(result["status"], "unsupported_in_rust")
        self.assertEqual(result["task_id"], "TEXT-EXTRA-001")
        self.assertFalse(result["equal"])

    def test_report_returns_nonzero_for_an_unsupported_candidate_api(self) -> None:
        expected_page = {
            "page_number": 1,
            "chars": [],
            "page_text": "",
            "layout_text": "",
            "simple_text": "",
            "text_lines": [],
            "words": [],
            "search": [],
            "tables": [],
            "annotations": [],
            "hyperlinks": [],
            "structure_tree": [],
        }
        actual_page = dict(expected_page)
        actual_page["simple_text"] = parity_report.unsupported_api("TEXT-EXTRA-001")
        expected = {"page_count": 1, "pages": [expected_page]}
        actual = {"page_count": 1, "pages": [actual_page]}

        with (
            mock.patch.object(parity_report.environment, "verify_reference"),
            mock.patch.object(parity_report, "find_fixtures", return_value=["/fixtures/a.pdf"]),
            mock.patch.object(parity_report, "python_side", return_value=expected),
            mock.patch.object(parity_report, "rust_side", return_value=actual),
            mock.patch("sys.argv", ["parity_report.py", "--fixtures", "/fixtures"]),
            mock.patch("builtins.print"),
        ):
            status = parity_report.main(object())

        self.assertEqual(status, 1)


if __name__ == "__main__":
    unittest.main()
