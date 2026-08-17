"""CI contracts separating semantic compatibility from performance (PARITY-023)."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
CI_WORKFLOW = REPO_ROOT / ".github" / "workflows" / "ci.yml"
COMPATIBILITY_TEST = (
    REPO_ROOT / "crates" / "pdfplumber" / "tests" / "compatibility_thresholds.rs"
)
LEGACY_MIXED_NAME = (
    REPO_ROOT / "crates" / "pdfplumber" / "tests" / "accuracy_benchmark.rs"
)
PERFORMANCE_BENCHMARK = (
    REPO_ROOT / "crates" / "pdfplumber" / "benches" / "extraction.rs"
)


def workflow_job(workflow: str, job_id: str) -> str:
    """Return one two-space-indented job block from the workflow."""

    match = re.search(
        rf"(?ms)^  {re.escape(job_id)}:\n(?P<body>.*?)(?=^  [a-z0-9_-]+:\n|\Z)",
        workflow,
    )
    if match is None:
        raise AssertionError(f"workflow job missing: {job_id}")
    return match.group(0)


class CompatibilityGateSeparationTests(unittest.TestCase):
    def test_semantic_gate_is_fail_closed_and_independent_of_benchmarks(self) -> None:
        workflow = CI_WORKFLOW.read_text(encoding="utf-8")
        job = workflow_job(workflow, "semantic-compatibility")

        self.assertIn("scripts/parity_report.py", job)
        self.assertIn("--json", job)
        self.assertIn("--summary", job)
        self.assertIn(".venv-reference/bin/python", job)
        self.assertNotIn("continue-on-error", job)
        self.assertNotIn("cargo bench", job)
        self.assertNotIn("benches/", job)
        self.assertNotIn("criterion", job.lower())

    def test_compatibility_diagnostics_are_tests_not_performance_benchmarks(self) -> None:
        self.assertTrue(COMPATIBILITY_TEST.is_file())
        self.assertFalse(LEGACY_MIXED_NAME.exists())
        self.assertTrue(PERFORMANCE_BENCHMARK.is_file())

        compatibility_source = COMPATIBILITY_TEST.read_text(encoding="utf-8")
        benchmark_source = PERFORMANCE_BENCHMARK.read_text(encoding="utf-8")
        self.assertIn("#[test]", compatibility_source)
        self.assertNotIn("Criterion", compatibility_source)
        self.assertIn("Criterion", benchmark_source)
        self.assertNotIn("CHAR_THRESHOLD", benchmark_source)
        self.assertNotIn("WORD_THRESHOLD", benchmark_source)


if __name__ == "__main__":
    unittest.main()
