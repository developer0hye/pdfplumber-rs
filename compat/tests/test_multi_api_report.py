"""Behavioral coverage for the per-API differential report."""

import json
import tempfile
import unittest
from pathlib import Path
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

    def test_report_returns_nonzero_for_unregistered_output_difference(self) -> None:
        expected_page = {
            "page_number": 1,
            "chars": [],
            "page_text": "upstream",
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
        actual_page["page_text"] = "rust"
        expected = {"page_count": 1, "pages": [expected_page]}
        actual = {"page_count": 1, "pages": [actual_page]}

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
            status = parity_report.main(object())

        self.assertEqual(status, 1)

    def test_report_accepts_only_the_exact_registered_output_difference(self) -> None:
        expected_page = self.empty_page()
        expected_page["page_text"] = "upstream"
        actual_page = dict(expected_page)
        actual_page["page_text"] = "rust"
        expected = {"page_count": 1, "pages": [expected_page]}
        actual = {"page_count": 1, "pages": [actual_page]}
        entry = parity_report.approved_deltas.ApprovedDelta(
            identifier="DELTA-001",
            fixture="a.pdf",
            page=1,
            api="page_text",
            upstream_result="upstream",
            upstream_sha256=parity_report.approved_deltas.value_digest("upstream"),
            rust_result="rust",
            rust_sha256=parity_report.approved_deltas.value_digest("rust"),
            technical_reason="intentional difference",
            compatibility_risk="different text is observable",
            approving_maintainer="developer0hye",
            regression_test="compat.tests.test_multi_api_report",
            review_condition="remove when outputs agree",
        )
        registry = parity_report.approved_deltas.Registry(
            version="0.11.10",
            commit="7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62",
            deltas=(entry,),
        )

        with (
            mock.patch.object(parity_report.environment, "verify_reference"),
            mock.patch.object(
                parity_report.approved_deltas,
                "load_registry",
                return_value=registry,
            ),
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
            status = parity_report.main(object())

        self.assertEqual(status, 0)

    def test_json_output_is_the_versioned_machine_report(self) -> None:
        expected_page = self.empty_page()
        expected = {"page_count": 1, "pages": [expected_page]}

        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "parity.json"
            with (
                mock.patch.object(parity_report.environment, "verify_reference"),
                mock.patch.object(
                    parity_report,
                    "find_fixtures",
                    return_value=["/fixtures/a.pdf"],
                ),
                mock.patch.object(parity_report, "python_side", return_value=expected),
                mock.patch.object(parity_report, "rust_side", return_value=expected),
                mock.patch(
                    "sys.argv",
                    [
                        "parity_report.py",
                        "--fixtures",
                        "/fixtures",
                        "--json",
                        str(output),
                    ],
                ),
                mock.patch("builtins.print"),
            ):
                status = parity_report.main(object())

            report = json.loads(output.read_text(encoding="utf-8"))

        self.assertEqual(status, 1)
        self.assertEqual(report["schema_version"], 1)
        self.assertEqual(report["fixtures"][0]["fixture_id"], "a.pdf")
        self.assertEqual(
            set(report["fixtures"][0]["pages"][0]["apis"]),
            set(parity_report.machine_report.PAGE_APIS),
        )
        self.assertEqual(report["summary"]["option_cases_total"], 161)
        self.assertEqual(report["summary"]["option_cases_blocked"], 161)
        self.assertEqual(
            report["options"][0]["candidate"],
            {"status": "blocked", "task_id": "PYAPI-002"},
        )

    def test_summary_output_shows_the_first_differing_object(self) -> None:
        upstream_page = self.empty_page()
        upstream_page["chars"] = [
            {
                "text": "A",
                "x0": 10.0,
                "x1": 15.0,
                "top": 20.0,
                "bottom": 30.0,
            }
        ]
        rust_page = dict(upstream_page)
        rust_page["chars"] = [
            {
                "text": "B",
                "x0": 10.25,
                "x1": 15.0,
                "top": 21.5,
                "bottom": 30.0,
            }
        ]
        expected = {"page_count": 1, "pages": [upstream_page]}
        actual = {"page_count": 1, "pages": [rust_page]}

        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "parity-summary.md"
            with (
                mock.patch.object(parity_report.environment, "verify_reference"),
                mock.patch.object(
                    parity_report,
                    "find_fixtures",
                    return_value=["/fixtures/object.pdf"],
                ),
                mock.patch.object(parity_report, "python_side", return_value=expected),
                mock.patch.object(parity_report, "rust_side", return_value=actual),
                mock.patch(
                    "sys.argv",
                    [
                        "parity_report.py",
                        "--fixtures",
                        "/fixtures",
                        "--summary",
                        str(output),
                    ],
                ),
                mock.patch("builtins.print"),
            ):
                status = parity_report.main(object())

            rendered = output.read_text(encoding="utf-8")

        self.assertEqual(status, 1)
        self.assertIn("# pdfplumber-rs parity summary", rendered)
        self.assertIn("- Fixture: `object.pdf`", rendered)
        self.assertIn("- API: `chars`", rendered)
        self.assertIn("- Object index: 0", rendered)
        self.assertIn('- Text: upstream `"A"` -> Rust `"B"`', rendered)
        self.assertIn("x0 `10.0` -> `10.25` (delta `+0.25`)", rendered)

    @staticmethod
    def empty_page() -> dict:
        return {
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


if __name__ == "__main__":
    unittest.main()
