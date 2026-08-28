"""Pinned cross-project benchmark contracts (SCORE-003)."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from compat.harness import benchmark_competitors

REPO_ROOT = Path(__file__).resolve().parents[2]
SUITE_PATH = REPO_ROOT / "benchmarks" / "competitors-v0.3.0.toml"
CORPUS_PATH = REPO_ROOT / "benchmarks" / "corpus-v0.3.0.toml"
POLICY_PATH = REPO_ROOT / "benchmarks" / "equivalence-v0.3.0.toml"
REGISTRY_PATH = REPO_ROOT / "compat" / "fixture-provenance.toml"
SCRIPT_PATH = REPO_ROOT / "scripts" / "run_competitor_benchmarks.py"
REPORT_PATH = REPO_ROOT / "docs" / "benchmarks" / "competitors-v0.3.0.md"


class CompetitorBenchmarkContractTests(unittest.TestCase):
    def test_repository_suite_pins_every_required_implementation(self) -> None:
        suite = benchmark_competitors.audit_repository(
            REPO_ROOT,
            SUITE_PATH,
            CORPUS_PATH,
            POLICY_PATH,
            REGISTRY_PATH,
        )

        self.assertEqual(suite.id, "pdfplumber-rs-competitors-v0.3.0")
        implementations = {
            implementation.id: implementation
            for implementation in suite.implementations
        }
        self.assertEqual(
            set(implementations),
            {
                "pdf-oxide",
                "pdfplumber-python",
                "pdfplumber-rs",
                "pdfsink-rs",
            },
        )
        self.assertEqual(
            implementations["pdfplumber-python"].revision,
            "7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62",
        )
        self.assertEqual(
            implementations["pdf-oxide"].revision,
            "3be1951b171edb9d69a10f42ef72ee73f52e51bf",
        )
        self.assertEqual(
            implementations["pdfsink-rs"].revision,
            "980d9f7b8ec44456f3d54427f4ced747b6eb6154",
        )
        self.assertEqual(implementations["pdfplumber-rs"].revision, "repository-head")
        for implementation in implementations.values():
            self.assertEqual(
                implementation.workloads,
                ("document-open", "text"),
            )
            self.assertNotIn("sh", implementation.command[0])

    def test_suite_expands_identical_digest_bound_cases(self) -> None:
        suite = benchmark_competitors.audit_repository(
            REPO_ROOT,
            SUITE_PATH,
            CORPUS_PATH,
            POLICY_PATH,
            REGISTRY_PATH,
        )
        cases = benchmark_competitors.expand_cases(suite)

        by_implementation: dict[str, set[tuple[str, str, str]]] = {}
        for case in cases:
            by_implementation.setdefault(case.implementation_id, set()).add(
                (case.fixture_id, case.fixture_sha256, case.workload_id)
            )
        self.assertEqual(len(by_implementation), 4)
        first, *rest = by_implementation.values()
        self.assertTrue(first)
        self.assertTrue(
            all(cases_for_implementation == first for cases_for_implementation in rest)
        )
        self.assertEqual(
            {workload_id for _, _, workload_id in first},
            {"document-open", "text"},
        )

    def test_timing_plan_requires_candidate_and_competitor_preflight_success(
        self,
    ) -> None:
        records = {
            implementation_id: benchmark_competitors.synthetic_record(
                implementation_id=implementation_id,
                revision="a" * 40,
                fixture_id="small-text",
                fixture_sha256=(
                    "22b6f9bd4aa388d7e6fb116d45bbe15e6da84c8d23fe20582d857e4b05c809ec"
                ),
                workload_id="text",
                outcome={
                    "status": "success",
                    "value": [{"page_number": 1, "text": "Benchmark text"}],
                },
            )
            for implementation_id in (
                "pdfplumber-python",
                "pdfplumber-rs",
                "pdf-oxide",
                "pdfsink-rs",
            )
        }
        eligible = benchmark_competitors.build_timing_plan(records)
        self.assertEqual(
            eligible,
            (
                ("pdfplumber-python", "pdfplumber-rs", "pdf-oxide"),
                ("pdfplumber-python", "pdfplumber-rs", "pdfsink-rs"),
            ),
        )

        records["pdfplumber-rs"]["outcome"]["value"][0]["text"] = "different"
        self.assertEqual(benchmark_competitors.build_timing_plan(records), ())

        records["pdfplumber-rs"]["outcome"]["value"][0]["text"] = "Benchmark text"
        records["pdf-oxide"]["outcome"] = {
            "status": "unsupported",
            "reason": "materially equivalent text options are unavailable",
        }
        self.assertEqual(
            benchmark_competitors.build_timing_plan(records),
            (("pdfplumber-python", "pdfplumber-rs", "pdfsink-rs"),),
        )

    def test_cli_check_validates_sources_without_running_or_timing_adapters(
        self,
    ) -> None:
        completed = subprocess.run(
            [sys.executable, str(SCRIPT_PATH), "--check"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("4 pinned implementations", completed.stdout)
        self.assertNotRegex(completed.stdout, r'"(?:duration|elapsed|latency)')
        self.assertEqual(
            REPORT_PATH.read_text(encoding="utf-8"),
            benchmark_competitors.render_markdown(
                benchmark_competitors.audit_repository(
                    REPO_ROOT,
                    SUITE_PATH,
                    CORPUS_PATH,
                    POLICY_PATH,
                    REGISTRY_PATH,
                )
            ),
        )
        self.assertIn(
            "python scripts/run_competitor_benchmarks.py --check",
            (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
                encoding="utf-8"
            ),
        )
        self.assertIn(
            "docs/benchmarks/competitors-v0.3.0.md",
            (REPO_ROOT / "README.md").read_text(encoding="utf-8"),
        )
        comparison = (REPO_ROOT / "docs" / "comparison.md").read_text(encoding="utf-8")
        self.assertIn("competitors-v0.3.0.md", comparison)
        self.assertIn("not published independently", comparison)
        self.assertIn("benchmarks/results-v0.3.0.md", comparison)

    def test_saved_run_keeps_rejections_untimed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            output_path = Path(temporary_directory) / "run.json"
            rejected_record = benchmark_competitors.synthetic_record(
                implementation_id="pdf-oxide",
                revision="a" * 40,
                fixture_id="small-text",
                fixture_sha256=(
                    "22b6f9bd4aa388d7e6fb116d45bbe15e6da84c8d23fe20582d857e4b05c809ec"
                ),
                workload_id="text",
                outcome={
                    "status": "unsupported",
                    "reason": "materially equivalent text options are unavailable",
                },
            )
            benchmark_competitors.write_local_run(
                output_path,
                records=[rejected_record],
                preflight_decisions=[
                    {
                        "case_id": "small-text:text",
                        "implementation_id": "pdf-oxide",
                        "eligible_for_timing": False,
                        "reasons": ["candidate outcome is unsupported"],
                    }
                ],
                timings=[],
            )

            saved = json.loads(output_path.read_text(encoding="utf-8"))
            self.assertEqual(saved["records"], [rejected_record])
            self.assertEqual(saved["timings"], [])
            self.assertFalse(saved["preflight_decisions"][0]["eligible_for_timing"])


if __name__ == "__main__":
    unittest.main()
