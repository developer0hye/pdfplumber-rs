"""Contract for exceptional/resource behavior differentials (PARITY-012)."""

from __future__ import annotations

import hashlib
import json
import unittest
from pathlib import Path

from compat.harness import error_contract, lockfile, upstream


CONTRACT_PATH: Path = (
    upstream.REPO_ROOT
    / "compat"
    / "contracts"
    / "pdfplumber-v0.11.10-error-behavior.json"
)

EXPECTED_CASES: frozenset[str] = frozenset(
    {
        "bbox.entirely_outside",
        "bbox.negative_height",
        "bbox.negative_width",
        "bbox.partially_outside",
        "bbox.strict_false",
        "bbox.zero_area",
        "closed.external_stream",
        "closed.initial_input",
        "closed.owned_stream",
        "exception.empty_pdf",
        "exception.table_settings_strategy",
        "metadata.non_strict_cycle",
        "metadata.strict_cycle",
        "password.correct",
        "password.missing",
        "password.wrong",
        "repair.bytesio_return",
        "repair.open_internal_stream",
        "repair.outfile",
        "repair.process_failure",
        "warning.annotation_unicode_fatal",
        "warning.annotation_unicode_nonfatal",
        "warning.deprecated_vertical_ttb",
    }
)
EXPECTED_CATEGORIES: frozenset[str] = frozenset(
    {
        "closed_resources",
        "exceptions",
        "invalid_bounding_boxes",
        "malformed_metadata",
        "passwords",
        "repair",
        "warnings",
    }
)
FIXTURES: tuple[str, ...] = (
    "tests/fixtures/generated/basic_text.pdf",
    "crates/pdfplumber/tests/fixtures/pdfs/annotations-unicode-issues.pdf",
    "crates/pdfplumber/tests/fixtures/pdfs/password-example.pdf",
)


class ErrorBehaviorContractTests(unittest.TestCase):
    def test_contract_covers_every_required_behavior_family(self) -> None:
        cases: tuple[error_contract.Case, ...] = error_contract.cases()
        self.assertEqual({case.identifier for case in cases}, set(EXPECTED_CASES))
        self.assertEqual({case.category for case in cases}, set(EXPECTED_CATEGORIES))
        self.assertEqual(
            [case.identifier for case in cases],
            sorted(EXPECTED_CASES),
            "case order must be deterministic",
        )
        for case in cases:
            self.assertTrue(case.callable_name)
            self.assertTrue(case.initial_phase)
            self.assertNotIn("test", case.invocation.values())
            self.assertNotIn("wrong", case.invocation.values())

    def test_committed_contract_is_complete_and_traceable(self) -> None:
        self.assertTrue(
            CONTRACT_PATH.is_file(),
            f"missing error contract: {CONTRACT_PATH.relative_to(upstream.REPO_ROOT)}",
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
            environment["python_version"], upstream.load_environment().python_version
        )

        resources: dict[str, dict[str, str]] = contract["resources"]  # type: ignore[assignment]
        for relative_path in FIXTURES:
            fixture: Path = upstream.REPO_ROOT / relative_path
            self.assertEqual(
                resources[relative_path]["sha256"],
                hashlib.sha256(fixture.read_bytes()).hexdigest(),
            )
        self.assertRegex(
            resources["inline:cyclic-metadata.pdf"]["sha256"], r"^[0-9a-f]{64}$"
        )
        self.assertRegex(
            resources["inline:repair-helper.py"]["sha256"], r"^[0-9a-f]{64}$"
        )

        records: list[dict[str, object]] = contract["cases"]  # type: ignore[assignment]
        self.assertEqual([record["id"] for record in records], sorted(EXPECTED_CASES))
        self.assertEqual({record["category"] for record in records}, set(EXPECTED_CATEGORIES))
        for record in records:
            for field in (
                "callable",
                "category",
                "invocation",
                "logs",
                "outcome",
                "phase",
                "warnings",
            ):
                self.assertIn(field, record, record["id"])
            outcome: dict[str, object] = record["outcome"]  # type: ignore[assignment]
            self.assertIn(outcome["kind"], {"return", "exception"})

        by_id: dict[str, dict[str, object]] = {
            str(record["id"]): record for record in records
        }
        expected_exception_types = {
            "closed.initial_input": "pdfplumber.utils.exceptions.PdfminerException",
            "exception.empty_pdf": "pdfplumber.utils.exceptions.PdfminerException",
            "metadata.strict_cycle": "builtins.RecursionError",
            "password.missing": "pdfplumber.utils.exceptions.PdfminerException",
            "password.wrong": "pdfplumber.utils.exceptions.PdfminerException",
            "repair.process_failure": "builtins.Exception",
            "warning.annotation_unicode_fatal": "builtins.UnicodeDecodeError",
        }
        for case_id, expected_type in expected_exception_types.items():
            self.assertEqual(self._exception_type(by_id, case_id), expected_type)

        nonfatal = by_id["warning.annotation_unicode_nonfatal"]
        self.assertEqual(nonfatal["outcome"]["kind"], "return")  # type: ignore[index]
        self.assertEqual(len(nonfatal["warnings"]), 1)  # type: ignore[arg-type]
        self.assertEqual(
            nonfatal["warnings"][0]["category"],  # type: ignore[index]
            "builtins.UserWarning",
        )
        self.assertEqual(
            len(by_id["metadata.non_strict_cycle"]["logs"]),  # type: ignore[arg-type]
            1,
        )
        self.assertEqual(
            len(
                by_id["warning.deprecated_vertical_ttb"]["logs"]  # type: ignore[arg-type]
            ),
            1,
        )

        for case_id in (
            "bbox.entirely_outside",
            "bbox.negative_height",
            "bbox.negative_width",
            "bbox.partially_outside",
            "bbox.zero_area",
            "exception.table_settings_strategy",
        ):
            self.assertEqual(self._exception_type(by_id, case_id), "builtins.ValueError")
        self.assertEqual(
            by_id["bbox.strict_false"]["outcome"]["kind"],  # type: ignore[index]
            "return",
        )
        self.assertEqual(
            by_id["password.correct"]["outcome"]["kind"],  # type: ignore[index]
            "return",
        )
        fatal = by_id["warning.annotation_unicode_fatal"]
        self.assertIn("byte 0x80", fatal["outcome"]["message"])  # type: ignore[index]

        serialized: str = json.dumps(contract, sort_keys=True)
        self.assertNotIn(str(upstream.REPO_ROOT), serialized)
        self.assertIsNone(
            error_contract._MEMORY_ADDRESS.search(serialized),
            "contract contains a memory address",
        )
        self.assertNotIn('"password": "test"', serialized)
        self.assertNotIn('"password": "wrong"', serialized)

    @staticmethod
    def _exception_type(
        records: dict[str, dict[str, object]], case_id: str
    ) -> object:
        outcome: dict[str, object] = records[case_id]["outcome"]  # type: ignore[assignment]
        return outcome["type"]


if __name__ == "__main__":
    unittest.main()
