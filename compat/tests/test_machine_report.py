"""Contracts for the versioned machine-readable parity report (PARITY-017)."""

from __future__ import annotations

import hashlib
import json
import unittest
from copy import deepcopy

from compat.harness import approved_deltas, machine_report


class MachineReportContractTests(unittest.TestCase):
    def test_report_retains_every_fixture_page_api_and_option_outcome(self) -> None:
        option_result = [{"text": "The"}]
        option_digest = self.result_digest(option_result)
        page = {
            "page_number": 1,
            "status": "compared",
            "chars": {
                "equal": False,
                "count_expected": 1,
                "count_actual": 1,
                "text_order_equal": True,
                "box_order_equal": False,
                "dictionary": {"structure_equal": True},
            },
            "words": {"equal": True},
            "page_text": {"equal": False},
            "layout_text": {"equal": True},
            "simple_text": {
                "status": "unsupported_in_rust",
                "task_id": "TEXT-002",
                "equal": False,
            },
            "text_lines": {"equal": True},
            "search": {"equal": True},
            "tables": {"equal": True},
            "annotations": {"equal": True},
            "hyperlinks": {"equal": True},
            "structure_tree": {"equal": True},
        }
        fixtures = {
            "tests/fixtures/z.pdf": {
                "status": "compared",
                "page_count_expected": 1,
                "page_count_actual": 1,
                "page_count_equal": True,
                "pages": [page],
            },
            "tests/fixtures/a.pdf": {
                "status": "python_failed",
                "error": "password required",
            },
        }
        option_snapshot = {
            "schema_version": 1,
            "target": {
                "project": "pdfplumber",
                "version": "0.11.10",
                "commit": "7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62",
            },
            "outputs": {option_digest: option_result},
            "cases": [
                {
                    "id": "text.extract_words.x_tolerance",
                    "domain": "text",
                    "api": "extract_words",
                    "fixture_path": "tests/fixtures/generated/basic_text.pdf",
                    "fixture_sha256": "f" * 64,
                    "page_number": 1,
                    "covers": ["extract_words.x_tolerance"],
                    "options": {"x_tolerance": 1},
                    "arguments": {},
                    "status": "ok",
                    "result": {"$ref": option_digest},
                    "warnings": [],
                    "logs": [],
                }
            ],
        }

        observation = approved_deltas.ObservedDelta(
            fixture="tests/fixtures/z.pdf",
            page=1,
            api="page_text",
            upstream_sha256="1" * 64,
            rust_sha256="2" * 64,
        )
        gate = approved_deltas.evaluate(
            (observation,),
            approved_deltas.Registry(
                version="0.11.10",
                commit="7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62",
                deltas=(),
            ),
        )

        report = machine_report.build(
            fixtures,
            option_snapshot,
            delta_gate=gate,
        )

        self.assertEqual(report["schema_version"], 1)
        self.assertEqual(
            report["candidate_environment"],
            {"status": "blocked", "task_id": "PYAPI-002"},
        )
        self.assertEqual(
            [fixture["fixture_id"] for fixture in report["fixtures"]],
            ["tests/fixtures/a.pdf", "tests/fixtures/z.pdf"],
        )
        compared = report["fixtures"][1]
        self.assertEqual(compared["pages"][0]["page_number"], 1)
        self.assertEqual(
            list(compared["pages"][0]["apis"]),
            list(machine_report.PAGE_APIS),
        )
        option = report["options"][0]
        self.assertEqual(option["options"], {"x_tolerance": 1})
        self.assertEqual(option["reference"]["result"], [{"text": "The"}])
        self.assertEqual(
            option["candidate"],
            {"status": "blocked", "task_id": "PYAPI-002"},
        )
        self.assertEqual(option["comparison"]["status"], "not_compared")
        self.assertEqual(report["summary"]["fixtures_total"], 2)
        self.assertEqual(report["summary"]["pages_compared"], 1)
        self.assertEqual(report["summary"]["api_results_unsupported"], 1)
        self.assertEqual(report["summary"]["api_results_different"], 2)
        self.assertEqual(report["summary"]["option_cases_total"], 1)
        self.assertEqual(report["summary"]["option_cases_compared"], 0)
        self.assertEqual(report["status"], "failed")
        self.assertEqual(
            compared["pages"][0]["apis"]["page_text"]["delta_gate"]["status"],
            "unregistered",
        )
        self.assertEqual(report["approved_delta_gate"]["unregistered"], 1)

        first = machine_report.render(report)
        self.assertEqual(first, machine_report.render(report))
        self.assertEqual(json.loads(first), report)
        self.assertNotIn("generated_at", report)

    def test_candidate_option_outcomes_are_exact_and_complete(self) -> None:
        reference_result = [{"text": "The"}]
        reference = self.option_snapshot(
            self.result_digest(reference_result),
            reference_result,
        )
        candidate = deepcopy(reference)

        equal = machine_report.build({}, reference, candidate)

        self.assertEqual(equal["summary"]["option_cases_compared"], 1)
        self.assertEqual(equal["summary"]["option_cases_equal"], 1)
        self.assertEqual(equal["candidate_environment"]["status"], "provided")
        self.assertEqual(equal["status"], "passed")

        candidate_result = [{"text": "Different"}]
        candidate_digest = self.result_digest(candidate_result)
        candidate["outputs"] = {candidate_digest: candidate_result}
        candidate["cases"][0]["result"] = {"$ref": candidate_digest}
        different = machine_report.build({}, reference, candidate)
        self.assertEqual(different["summary"]["option_cases_different"], 1)
        self.assertEqual(different["status"], "failed")

        candidate["cases"] = []
        with self.assertRaisesRegex(
            machine_report.MachineReportError,
            "candidate option IDs differ",
        ):
            machine_report.build({}, reference, candidate)

    def test_option_result_references_must_be_sha256_digests(self) -> None:
        invalid = self.option_snapshot("not-a-digest", [{"text": "The"}])

        with self.assertRaisesRegex(machine_report.MachineReportError, "SHA-256"):
            machine_report.build({}, invalid)

        mismatched = self.option_snapshot("a" * 64, [{"text": "The"}])
        with self.assertRaisesRegex(
            machine_report.MachineReportError,
            "does not match",
        ):
            machine_report.build({}, mismatched)

    @staticmethod
    def result_digest(result: object) -> str:
        encoded = json.dumps(
            result,
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
        return hashlib.sha256(encoded).hexdigest()

    @staticmethod
    def option_snapshot(result_ref: str, result: object) -> dict[str, object]:
        return {
            "schema_version": 1,
            "target": {
                "project": "pdfplumber",
                "version": "0.11.10",
                "commit": "7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62",
            },
            "outputs": {result_ref: result},
            "cases": [
                {
                    "id": "text.extract_words.x_tolerance",
                    "domain": "text",
                    "api": "extract_words",
                    "fixture_path": "tests/fixtures/generated/basic_text.pdf",
                    "fixture_sha256": "f" * 64,
                    "page_number": 1,
                    "covers": ["extract_words.x_tolerance"],
                    "options": {"x_tolerance": 1},
                    "arguments": {},
                    "status": "ok",
                    "result": {"$ref": result_ref},
                    "warnings": [],
                    "logs": [],
                }
            ],
        }


if __name__ == "__main__":
    unittest.main()
