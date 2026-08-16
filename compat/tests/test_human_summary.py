"""Contracts for the human-readable parity summary (PARITY-018)."""

from __future__ import annotations

import unittest

from compat.harness import human_summary
from scripts import parity_report


class HumanSummaryContractTests(unittest.TestCase):
    def test_comparisons_retain_the_first_object_and_text_difference(self) -> None:
        upstream_char = {
            "text": "A",
            "x0": 10.0,
            "x1": 15.0,
            "top": 20.0,
            "bottom": 30.0,
        }
        rust_char = {
            "text": "B",
            "x0": 10.25,
            "x1": 15.0,
            "top": 21.5,
            "bottom": 30.0,
        }

        chars = parity_report.compare_chars([upstream_char], [rust_char])
        annotations = parity_report.compare_api_value(
            [{"title": "upstream", "x0": 1.0}],
            [{"title": "rust", "x0": 2.0}],
        )
        text = parity_report.compare_text(
            "line one\nupstream tail",
            "line one\nRust tail",
        )

        self.assertEqual(
            chars["first_difference"],
            {
                "kind": "sequence",
                "index": 0,
                "upstream_present": True,
                "rust_present": True,
                "upstream": upstream_char,
                "rust": rust_char,
            },
        )
        self.assertEqual(annotations["first_difference"]["index"], 0)
        self.assertEqual(
            annotations["first_difference"]["upstream"],
            {"title": "upstream", "x0": 1.0},
        )
        self.assertEqual(
            text["first_difference"],
            {
                "kind": "text",
                "index": 9,
                "upstream_present": True,
                "rust_present": True,
                "upstream": "u",
                "rust": "R",
                "upstream_context": "line one\nupstream tail",
                "rust_context": "line one\nRust tail",
                "context_start": 0,
            },
        )

    def test_summary_shows_first_differing_object_text_and_coordinates(self) -> None:
        upstream = {
            "text": "A",
            "x0": 10.0,
            "x1": 15.0,
            "top": 20.0,
            "bottom": 30.0,
            "fontname": "Alpha",
        }
        rust = {
            "text": "B",
            "x0": 10.25,
            "x1": 15.0,
            "top": 21.5,
            "bottom": 30.0,
            "fontname": "Beta",
        }
        report = self.report(
            [
                self.fixture(
                    "tests/fixtures/generated/object.pdf",
                    "chars",
                    {
                        "kind": "sequence",
                        "index": 0,
                        "upstream": upstream,
                        "rust": rust,
                    },
                )
            ]
        )

        rendered = human_summary.render(report)

        self.assertIn("# pdfplumber-rs parity summary", rendered)
        self.assertIn("- Fixture: `tests/fixtures/generated/object.pdf`", rendered)
        self.assertIn("- Page: 1", rendered)
        self.assertIn("- API: `chars`", rendered)
        self.assertIn("- Object index: 0", rendered)
        self.assertIn('- Text: upstream `"A"` -> Rust `"B"`', rendered)
        self.assertIn(
            "- Coordinates: x0 `10.0` -> `10.25` (delta `+0.25`); "
            "top `20.0` -> `21.5` (delta `+1.5`)",
            rendered,
        )
        self.assertIn('- Upstream object: `{"bottom":30.0,', rendered)
        self.assertIn('- Rust object: `{"bottom":30.0,', rendered)
        self.assertEqual(rendered, human_summary.render(report))

    def test_summary_uses_stable_first_result_and_compact_text_context(self) -> None:
        first = self.fixture(
            "tests/fixtures/generated/a-text.pdf",
            "page_text",
            {
                "kind": "text",
                "index": 12,
                "upstream": "x",
                "rust": "X",
                "upstream_context": "line one\\ntext",
                "rust_context": "line one\\nText",
            },
            page_number=2,
        )
        later = self.fixture(
            "tests/fixtures/generated/z-words.pdf",
            "words",
            {
                "kind": "sequence",
                "index": 4,
                "upstream": {"text": "later", "x0": 1.0},
                "rust": {"text": "ignored", "x0": 2.0},
            },
        )
        report = self.report([first, later])

        rendered = human_summary.render(report)
        first_section = rendered.split("## First differing result\n", 1)[1]

        self.assertIn("- Fixture: `tests/fixtures/generated/a-text.pdf`", first_section)
        self.assertIn("- Page: 2", first_section)
        self.assertIn("- API: `page_text`", first_section)
        self.assertIn("- Text offset: 12", first_section)
        self.assertIn(
            '- Text: upstream `"line one\\\\ntext"` -> Rust `"line one\\\\nText"`',
            first_section,
        )
        self.assertNotIn("ignored", first_section)

    def test_summary_states_when_common_coordinates_do_not_differ(self) -> None:
        report = self.report(
            [
                self.fixture(
                    "tests/fixtures/generated/schema-only.pdf",
                    "chars",
                    {
                        "kind": "sequence",
                        "index": 0,
                        "upstream": {"text": "D", "x0": 23.76, "top": 760.893},
                        "rust": {"text": "D", "x0": 23.76, "top": 760.893},
                    },
                )
            ]
        )

        rendered = human_summary.render(report)

        self.assertIn(
            "- Coordinates: no differing common coordinates",
            rendered,
        )

    @staticmethod
    def fixture(
        fixture_id: str,
        api: str,
        difference: dict[str, object],
        page_number: int = 1,
    ) -> dict[str, object]:
        return {
            "fixture_id": fixture_id,
            "status": "compared",
            "pages": [
                {
                    "page_number": page_number,
                    "status": "compared",
                    "apis": {
                        api: {
                            "status": "different",
                            "comparison": {"first_difference": difference},
                        }
                    },
                }
            ],
        }

    @staticmethod
    def report(fixtures: list[dict[str, object]]) -> dict[str, object]:
        return {
            "schema_version": 1,
            "target": {
                "project": "pdfplumber",
                "version": "0.11.10",
                "commit": "7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62",
            },
            "status": "failed",
            "summary": {
                "fixtures_total": len(fixtures),
                "fixtures_failed": 0,
                "pages_compared": len(fixtures),
                "api_results_equal": 0,
                "api_results_different": len(fixtures),
                "api_results_unsupported": 0,
                "option_cases_total": 161,
                "option_cases_equal": 0,
                "option_cases_different": 0,
                "option_cases_blocked": 161,
            },
            "approved_delta_gate": {
                "status": "failed",
                "approved": 0,
                "unregistered": len(fixtures),
                "stale": 0,
                "stale_entries": [],
            },
            "fixtures": fixtures,
            "options": [],
        }


if __name__ == "__main__":
    unittest.main()
