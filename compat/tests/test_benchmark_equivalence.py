"""Output-equivalence preflight contracts (SCORE-002)."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from copy import deepcopy
from pathlib import Path

import tomllib

from compat.harness import benchmark_corpus, benchmark_equivalence

REPO_ROOT = Path(__file__).resolve().parents[2]
CORPUS_PATH = REPO_ROOT / "benchmarks" / "corpus-v0.3.0.toml"
POLICY_PATH = REPO_ROOT / "benchmarks" / "equivalence-v0.3.0.toml"
REGISTRY_PATH = REPO_ROOT / "compat" / "fixture-provenance.toml"
REPORT_PATH = REPO_ROOT / "docs" / "benchmarks" / "equivalence-v0.3.0.md"
SCRIPT_PATH = REPO_ROOT / "scripts" / "check_benchmark_equivalence.py"


def text_record(
    implementation_id: str,
    *,
    request: dict[str, object] | None = None,
    value: object | None = None,
    status: str = "success",
) -> dict[str, object]:
    outcome: dict[str, object]
    if status == "success":
        outcome = {
            "status": "success",
            "value": (
                [{"page_number": 1, "text": "Benchmark text"}]
                if value is None
                else value
            ),
        }
    elif status == "unsupported":
        outcome = {
            "status": "unsupported",
            "reason": "workload is not implemented",
        }
    else:
        outcome = {
            "status": "error",
            "error": {"kind": "parse", "message": "cannot extract text"},
        }
    return {
        "schema_version": 1,
        "implementation": {
            "id": implementation_id,
            "revision": "a" * 40 if implementation_id == "reference" else "b" * 40,
        },
        "fixture": {
            "id": "small-text",
            "sha256": "22b6f9bd4aa388d7e6fb116d45bbe15e6da84c8d23fe20582d857e4b05c809ec",
        },
        "workload": {
            "id": "text",
            "output_schema": "page-text-v1",
        },
        "request": request
        or {
            "layout": False,
            "normalization": "none",
            "page_selection": "all",
            "preserve_page_boundaries": True,
        },
        "outcome": outcome,
    }


class BenchmarkEquivalenceContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.corpus = benchmark_corpus.audit_repository(
            REPO_ROOT,
            CORPUS_PATH,
            REGISTRY_PATH,
        )
        self.policy = benchmark_equivalence.load_policy(POLICY_PATH)

    def test_repository_policy_covers_each_canonical_output_family(self) -> None:
        policy = benchmark_equivalence.audit_repository(
            REPO_ROOT,
            POLICY_PATH,
            CORPUS_PATH,
            REGISTRY_PATH,
        )

        self.assertEqual(policy.id, "pdfplumber-rs-equivalence-v0.3.0")
        self.assertEqual(policy.release, "0.3.0")
        self.assertEqual(policy.corpus_id, self.corpus.id)
        self.assertEqual(
            tuple(workload.id for workload in policy.workloads),
            (
                "document-open",
                "graphics",
                "images",
                "tables",
                "text",
                "words",
            ),
        )
        self.assertEqual(
            {workload.output_schema for workload in policy.workloads},
            {
                "page-count-v1",
                "page-graphics-v1",
                "page-images-v1",
                "page-tables-v1",
                "page-text-v1",
                "page-words-v1",
            },
        )

    def test_exact_canonical_output_is_eligible_for_timing(self) -> None:
        reference = text_record(
            "reference",
            value=[{"text": "Benchmark text", "page_number": 1}],
        )
        candidate = text_record("candidate")

        decision = benchmark_equivalence.preflight(
            reference,
            candidate,
            self.policy,
            self.corpus,
        )

        self.assertTrue(decision.eligible_for_timing)
        self.assertEqual(decision.reasons, ())
        self.assertEqual(
            decision.reference_output_sha256,
            decision.candidate_output_sha256,
        )
        self.assertNotRegex(
            json.dumps(decision.to_dict(), sort_keys=True),
            r'"(?:duration|elapsed|latency|time|timing)',
        )

    def test_output_value_and_json_number_type_differences_are_rejected(self) -> None:
        cases = (
            (
                "value",
                [{"page_number": 1, "text": "Different text"}],
            ),
            (
                "number type",
                [{"page_number": 1.0, "text": "Benchmark text"}],
            ),
            (
                "sequence",
                [
                    {"page_number": 1, "text": "Benchmark text"},
                    {"page_number": 2, "text": ""},
                ],
            ),
        )

        for case, value in cases:
            with self.subTest(case=case):
                decision = benchmark_equivalence.preflight(
                    text_record("reference"),
                    text_record("candidate", value=value),
                    self.policy,
                    self.corpus,
                )
                self.assertFalse(decision.eligible_for_timing)
                self.assertIn("canonical output differs", decision.reasons)

    def test_request_fixture_and_schema_mismatches_are_rejected(self) -> None:
        request = text_record("candidate")
        request["request"]["layout"] = True

        digest = text_record("candidate")
        digest["fixture"]["sha256"] = "0" * 64

        schema = text_record("candidate")
        schema["workload"]["output_schema"] = "plain-string-v1"

        cases = (
            ("request", request, "candidate request does not match workload contract"),
            ("digest", digest, "candidate fixture digest does not match corpus"),
            ("schema", schema, "candidate output schema does not match workload"),
        )
        for case, candidate, reason in cases:
            with self.subTest(case=case):
                decision = benchmark_equivalence.preflight(
                    text_record("reference"),
                    candidate,
                    self.policy,
                    self.corpus,
                )
                self.assertFalse(decision.eligible_for_timing)
                self.assertIn(reason, decision.reasons)

    def test_error_unsupported_and_same_implementation_cases_are_rejected(self) -> None:
        cases = (
            (
                "reference error",
                text_record("reference", status="error"),
                text_record("candidate"),
                "reference outcome is error",
            ),
            (
                "candidate unsupported",
                text_record("reference"),
                text_record("candidate", status="unsupported"),
                "candidate outcome is unsupported",
            ),
            (
                "same implementation",
                text_record("reference"),
                text_record("reference"),
                "implementations must be distinct",
            ),
        )

        for case, reference, candidate, reason in cases:
            with self.subTest(case=case):
                decision = benchmark_equivalence.preflight(
                    reference,
                    candidate,
                    self.policy,
                    self.corpus,
                )
                self.assertFalse(decision.eligible_for_timing)
                self.assertIn(reason, decision.reasons)

    def test_malformed_or_timed_records_fail_closed(self) -> None:
        extra_timing = text_record("candidate")
        extra_timing["elapsed_seconds"] = 0.001

        non_finite = text_record("candidate", value=float("nan"))

        missing_value = text_record("candidate")
        del missing_value["outcome"]["value"]

        cases = (
            ("timing", extra_timing, r"unexpected record fields: elapsed_seconds"),
            ("non-finite", non_finite, r"finite JSON number"),
            ("missing", missing_value, r"success outcome needs value"),
        )
        for case, candidate, message in cases:
            with (
                self.subTest(case=case),
                self.assertRaisesRegex(
                    benchmark_equivalence.BenchmarkEquivalenceError,
                    message,
                ),
            ):
                benchmark_equivalence.preflight(
                    text_record("reference"),
                    candidate,
                    self.policy,
                    self.corpus,
                )

    def test_policy_rejects_duplicate_missing_or_unsafe_workloads(self) -> None:
        with POLICY_PATH.open("rb") as policy_file:
            source = tomllib.load(policy_file)

        duplicate = deepcopy(source)
        duplicate["workloads"].append(deepcopy(duplicate["workloads"][0]))

        missing = deepcopy(source)
        missing["workloads"] = [
            workload for workload in missing["workloads"] if workload["id"] != "tables"
        ]

        unsafe = deepcopy(source)
        unsafe["workloads"][0]["semantic_classes"] = ["marketing"]

        cases = (
            ("duplicate", duplicate, r"duplicate workload id"),
            ("missing", missing, r"missing workloads: tables"),
            ("unsafe", unsafe, r"unknown semantic class: marketing"),
        )
        for case, policy, message in cases:
            with (
                self.subTest(case=case),
                self.assertRaisesRegex(
                    benchmark_equivalence.BenchmarkEquivalenceError,
                    message,
                ),
            ):
                benchmark_equivalence.validate_policy(policy, REPO_ROOT)

    def test_cli_blocks_a_mismatch_before_any_timing_result_exists(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            directory = Path(temporary_directory)
            reference_path = directory / "reference.json"
            candidate_path = directory / "candidate.json"
            reference_path.write_text(
                json.dumps(text_record("reference")),
                encoding="utf-8",
            )
            candidate = text_record("candidate")
            candidate_path.write_text(json.dumps(candidate), encoding="utf-8")

            passing = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT_PATH),
                    "--reference",
                    str(reference_path),
                    "--candidate",
                    str(candidate_path),
                ],
                cwd=REPO_ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(passing.returncode, 0, passing.stderr)
            self.assertTrue(json.loads(passing.stdout)["eligible_for_timing"])

            candidate["outcome"]["value"][0]["text"] = "Different text"
            candidate_path.write_text(json.dumps(candidate), encoding="utf-8")
            rejected = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT_PATH),
                    "--reference",
                    str(reference_path),
                    "--candidate",
                    str(candidate_path),
                ],
                cwd=REPO_ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(rejected.returncode, 1, rejected.stderr)
            self.assertFalse(json.loads(rejected.stdout)["eligible_for_timing"])
            self.assertNotRegex(rejected.stdout, r'"(?:duration|elapsed|latency)')

    def test_generated_report_reference_and_ci_gate_are_current(self) -> None:
        policy = benchmark_equivalence.audit_repository(
            REPO_ROOT,
            POLICY_PATH,
            CORPUS_PATH,
            REGISTRY_PATH,
        )
        self.assertEqual(
            REPORT_PATH.read_text(encoding="utf-8"),
            benchmark_equivalence.render_markdown(policy),
        )
        self.assertIn(
            "docs/benchmarks/equivalence-v0.3.0.md",
            (REPO_ROOT / "README.md").read_text(encoding="utf-8"),
        )
        self.assertIn(
            "python scripts/check_benchmark_equivalence.py --check",
            (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
                encoding="utf-8"
            ),
        )
        self.assertIn(
            "mlperf-inference.md",
            (REPO_ROOT / "references" / "INDEX.md").read_text(encoding="utf-8"),
        )


if __name__ == "__main__":
    unittest.main()
