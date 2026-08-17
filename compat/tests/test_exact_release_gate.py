"""Release-readiness contracts for exact API results (PARITY-026)."""

from __future__ import annotations

import hashlib
import json
import unittest
from copy import deepcopy
from pathlib import Path
from unittest import mock

from scripts import parity_report


REPO_ROOT = Path(__file__).resolve().parents[2]


class ExactReleaseGateTests(unittest.TestCase):
    def test_perfect_percentage_diagnostics_do_not_hide_exact_char_differences(
        self,
    ) -> None:
        mutations = {
            "coordinate.pdf": {"x0": 10.01},
            "font-metadata.pdf": {"fontname": "Courier"},
        }

        for fixture_id, mutation in mutations.items():
            with self.subTest(fixture=fixture_id):
                expected_page = self.page()
                actual_page = self.page()
                actual_page["chars"][0].update(mutation)
                expected = {"page_count": 1, "pages": [expected_page]}
                actual = {"page_count": 1, "pages": [actual_page]}

                comparison = parity_report.compare_documents(expected, actual)
                char_diagnostic = comparison["pages"][0]["chars"]
                observations = parity_report.observed_document_deltas(
                    fixture_id, expected, actual, comparison
                )

                self.assertEqual(char_diagnostic["text_ratio"], 1.0)
                self.assertTrue(char_diagnostic["text_order_equal"])
                self.assertTrue(char_diagnostic["box_order_equal"])
                self.assertTrue(char_diagnostic["dictionary"]["structure_equal"])
                self.assertEqual(len(observations), 1)
                self.assertEqual(observations[0].api, "chars")
                self.assertFalse(char_diagnostic["equal"])

    def test_release_gate_accepts_exact_char_difference_only_when_registered(
        self,
    ) -> None:
        expected_page = self.page()
        actual_page = self.page()
        actual_page["chars"][0]["x0"] = 10.01
        expected = {"page_count": 1, "pages": [expected_page]}
        actual = {"page_count": 1, "pages": [actual_page]}
        empty = parity_report.approved_deltas.Registry(
            version="0.11.10",
            commit="7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62",
            deltas=(),
        )

        self.assertEqual(self.run_gate(expected, actual, empty), 1)

        observed = parity_report.approved_deltas.ObservedDelta(
            fixture="exact.pdf",
            page=1,
            api="chars",
            upstream_sha256=parity_report.approved_deltas.value_digest(
                expected_page["chars"]
            ),
            rust_sha256=parity_report.approved_deltas.value_digest(
                actual_page["chars"]
            ),
        )
        approval = parity_report.approved_deltas.ApprovedDelta(
            identifier="DELTA-999",
            fixture=observed.fixture,
            page=observed.page,
            api=observed.api,
            upstream_result="x0=10.0",
            upstream_sha256=observed.upstream_sha256,
            rust_result="x0=10.01",
            rust_sha256=observed.rust_sha256,
            technical_reason="maintainer-reviewed exact coordinate difference",
            compatibility_risk="coordinate consumers observe a different value",
            approving_maintainer="developer0hye",
            regression_test="compat.tests.test_exact_release_gate",
            review_condition="remove when exact coordinates agree",
        )
        approved = parity_report.approved_deltas.Registry(
            version=empty.version,
            commit=empty.commit,
            deltas=(approval,),
        )

        self.assertEqual(self.run_gate(expected, actual, approved), 0)

    def test_runtime_type_coercion_is_an_exact_api_difference(self) -> None:
        expected_page = self.page()
        actual_page = self.page()
        expected_page["annotations"] = [{"enabled": True}]
        actual_page["annotations"] = [{"enabled": 1}]
        expected = {"page_count": 1, "pages": [expected_page]}
        actual = {"page_count": 1, "pages": [actual_page]}

        comparison = parity_report.compare_documents(expected, actual)
        observations = parity_report.observed_document_deltas(
            "types.pdf", expected, actual, comparison
        )

        self.assertFalse(comparison["pages"][0]["annotations"]["equal"])
        self.assertEqual(
            comparison["pages"][0]["annotations"]["first_difference"]["index"],
            0,
        )
        self.assertTrue(
            comparison["pages"][0]["annotations"]["first_difference"][
                "upstream_present"
            ]
        )
        self.assertTrue(
            comparison["pages"][0]["annotations"]["first_difference"][
                "rust_present"
            ]
        )
        self.assertEqual(
            [observation.api for observation in observations], ["annotations"]
        )

    def test_option_results_preserve_runtime_types(self) -> None:
        reference = self.option_snapshot(True)
        candidate = deepcopy(reference)
        candidate_digest = self.result_digest(1)
        candidate["outputs"] = {candidate_digest: 1}
        candidate["cases"][0]["result"] = {"$ref": candidate_digest}

        report = parity_report.machine_report.build({}, reference, candidate)

        self.assertEqual(report["summary"]["option_cases_different"], 1)
        self.assertEqual(report["status"], "failed")

    def test_release_workflow_reuses_the_exact_semantic_gate(self) -> None:
        ci = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        release = (REPO_ROOT / ".github" / "workflows" / "release.yml").read_text(
            encoding="utf-8"
        )
        semantic_job = ci.split("  semantic-compatibility:\n", 1)[1]

        self.assertIn("scripts/parity_report.py", semantic_job)
        self.assertNotIn("compatibility_thresholds", semantic_job)
        self.assertNotIn("95%", semantic_job)
        self.assertIn("uses: ./.github/workflows/ci.yml", release)
        self.assertIn("needs: ci", release)

    @staticmethod
    def run_gate(
        expected: dict, actual: dict, registry: parity_report.approved_deltas.Registry
    ) -> int:
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
                return_value=["/fixtures/exact.pdf"],
            ),
            mock.patch.object(parity_report, "python_side", return_value=expected),
            mock.patch.object(parity_report, "rust_side", return_value=actual),
            mock.patch(
                "sys.argv", ["parity_report.py", "--fixtures", "/fixtures"]
            ),
            mock.patch("builtins.print"),
        ):
            return parity_report.main(object())

    @staticmethod
    def page() -> dict:
        return {
            "page_number": 1,
            "chars": [
                {
                    "text": "A",
                    "x0": 10.0,
                    "top": 20.0,
                    "x1": 15.0,
                    "bottom": 30.0,
                    "fontname": "Helvetica",
                }
            ],
            "words": [],
            "page_text": "A",
            "layout_text": "A",
            "simple_text": "A",
            "text_lines": [],
            "search": [],
            "tables": [],
            "annotations": [],
            "hyperlinks": [],
            "structure_tree": [],
        }

    @classmethod
    def option_snapshot(cls, result: object) -> dict:
        digest = cls.result_digest(result)
        return {
            "schema_version": 1,
            "target": {
                "project": "pdfplumber",
                "version": "0.11.10",
                "commit": "7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62",
            },
            "outputs": {digest: result},
            "cases": [
                {
                    "id": "text.extract_words.types",
                    "domain": "text",
                    "api": "extract_words",
                    "fixture_path": "tests/fixtures/generated/types.pdf",
                    "fixture_sha256": "f" * 64,
                    "page_number": 1,
                    "covers": ["extract_words.types"],
                    "options": {},
                    "arguments": {},
                    "status": "ok",
                    "result": {"$ref": digest},
                    "warnings": [],
                    "logs": [],
                }
            ],
        }

    @staticmethod
    def result_digest(result: object) -> str:
        encoded = json.dumps(
            result,
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
        return hashlib.sha256(encoded).hexdigest()


if __name__ == "__main__":
    unittest.main()
