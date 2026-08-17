"""Behavioral call-contract gate for pinned upstream (PARITY-006)."""

from __future__ import annotations

import hashlib
import json
import unittest
from pathlib import Path

from compat.harness import lockfile, upstream


CONTRACT_PATH: Path = (
    upstream.REPO_ROOT
    / "compat"
    / "contracts"
    / "pdfplumber-v0.11.10-calls.json"
)
EXPECTED_CATEGORIES: set[str] = {
    "defaults",
    "exception_types",
    "invalid_arguments",
    "keyword_arguments",
    "keyword_only_arguments",
    "positional_arguments",
}
EXPECTED_CASES: set[str] = {
    "defaults.cluster_list",
    "defaults.table_settings",
    "exception.empty_pdf",
    "exception.table_settings_strategy",
    "invalid.missing_required",
    "invalid.too_many_positional",
    "invalid.unexpected_keyword",
    "keyword.cluster_list",
    "keyword_only.page_extract_text",
    "keyword_only.page_extract_text_rejects_positional",
    "positional.cluster_list",
    "positional_only.ctm_count",
}


class ApiCallContractTest(unittest.TestCase):
    def test_behavioral_contract_covers_every_call_category(self) -> None:
        self.assertTrue(
            CONTRACT_PATH.is_file(),
            f"missing pinned call contract: {CONTRACT_PATH.relative_to(upstream.REPO_ROOT)}",
        )
        contract: dict[str, object] = json.loads(
            CONTRACT_PATH.read_text(encoding="utf-8")
        )

        target: upstream.Target = upstream.load_target()
        self.assertEqual(contract["schema_version"], 1)
        self.assertEqual(
            contract["target"],
            {
                "project": target.project,
                "version": target.version,
                "tag": target.tag,
                "commit": target.commit,
                "repository": target.repository,
            },
        )

        environment: dict[str, object] = contract["environment"]  # type: ignore[assignment]
        self.assertEqual(environment["lockfile_sha256"], lockfile.digest())
        self.assertEqual(
            environment["python_version"],
            upstream.load_environment().python_version,
        )

        fixture: Path = (
            upstream.REPO_ROOT / "tests/fixtures/downloaded/pdffill-demo.pdf"
        )
        resources: dict[str, dict[str, str]] = contract["resources"]  # type: ignore[assignment]
        self.assertEqual(
            resources["tests/fixtures/downloaded/pdffill-demo.pdf"]["sha256"],
            hashlib.sha256(fixture.read_bytes()).hexdigest(),
        )

        cases: list[dict[str, object]] = contract["cases"]  # type: ignore[assignment]
        self.assertEqual({str(case["id"]) for case in cases}, EXPECTED_CASES)
        self.assertEqual({str(case["category"]) for case in cases}, EXPECTED_CATEGORIES)
        self.assertEqual(
            [str(case["id"]) for case in cases],
            sorted(EXPECTED_CASES),
            "case order must be deterministic",
        )

        by_id: dict[str, dict[str, object]] = {
            str(case["id"]): case for case in cases
        }
        self.assertEqual(
            by_id["positional.cluster_list"]["outcome"],
            {
                "kind": "return",
                "type": "builtins.list",
                "value": [[1, 2], [5, 6]],
            },
        )
        self.assertEqual(
            by_id["keyword.cluster_list"]["outcome"],
            by_id["positional.cluster_list"]["outcome"],
        )
        self.assertEqual(
            by_id["defaults.cluster_list"]["outcome"],
            {
                "kind": "return",
                "type": "builtins.list",
                "value": [[1], [2], [3], [4]],
            },
        )
        table_defaults: dict[str, object] = by_id["defaults.table_settings"][
            "outcome"
        ]  # type: ignore[assignment]
        self.assertEqual(table_defaults["kind"], "return")
        self.assertEqual(table_defaults["type"], "pdfplumber.table.TableSettings")
        self.assertEqual(
            table_defaults["value"],
            {
                "edge_min_length": 3,
                "edge_min_length_prefilter": 1,
                "explicit_horizontal_lines": None,
                "explicit_vertical_lines": None,
                "horizontal_strategy": "lines",
                "intersection_tolerance": 3,
                "intersection_x_tolerance": 3,
                "intersection_y_tolerance": 3,
                "join_tolerance": 3,
                "join_x_tolerance": 3,
                "join_y_tolerance": 3,
                "min_words_horizontal": 1,
                "min_words_vertical": 3,
                "snap_tolerance": 3,
                "snap_x_tolerance": 3,
                "snap_y_tolerance": 3,
                "text_settings": {"x_tolerance": 3, "y_tolerance": 3},
                "vertical_strategy": "lines",
            },
        )

        keyword_only: dict[str, object] = by_id[
            "keyword_only.page_extract_text"
        ]["outcome"]  # type: ignore[assignment]
        self.assertEqual(keyword_only["kind"], "return")
        self.assertEqual(keyword_only["type"], "builtins.str")
        keyword_only_summary: dict[str, object] = keyword_only["value"]  # type: ignore[assignment]
        self.assertEqual(keyword_only_summary["length"], 5099)
        self.assertEqual(keyword_only_summary["line_count"], 60)
        self.assertIs(keyword_only_summary["pdfill_heading_present"], True)

        self.assertEqual(
            by_id["positional_only.ctm_count"]["outcome"],
            {"kind": "return", "type": "builtins.int", "value": 4},
        )

        for case_id in (
            "invalid.missing_required",
            "invalid.too_many_positional",
            "invalid.unexpected_keyword",
        ):
            self.assertEqual(
                self._exception_type(by_id, case_id), "builtins.TypeError"
            )
        self.assertEqual(
            self._exception_type(
                by_id, "keyword_only.page_extract_text_rejects_positional"
            ),
            "builtins.TypeError",
        )
        self.assertEqual(
            self._exception_type(by_id, "exception.table_settings_strategy"),
            "builtins.ValueError",
        )
        self.assertEqual(
            self._exception_type(by_id, "exception.empty_pdf"),
            "pdfplumber.utils.exceptions.PdfminerException",
        )

        for case in cases:
            self.assertIn("callable", case)
            self.assertIn("invocation", case)
            outcome: dict[str, object] = case["outcome"]  # type: ignore[assignment]
            self.assertIn(outcome["kind"], {"return", "exception"})

        serialized: str = json.dumps(contract, sort_keys=True)
        self.assertNotIn(str(upstream.REPO_ROOT), serialized)
        self.assertNotIn("0x", serialized, "contract contains a memory address")

    @staticmethod
    def _exception_type(
        by_id: dict[str, dict[str, object]], case_id: str
    ) -> object:
        outcome: dict[str, object] = by_id[case_id]["outcome"]  # type: ignore[assignment]
        return outcome["type"]


if __name__ == "__main__":
    unittest.main()
