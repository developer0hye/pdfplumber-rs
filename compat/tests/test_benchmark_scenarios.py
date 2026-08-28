"""State, page-scope, and parallel workload contracts (SCORE-006)."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from compat.harness import benchmark_scenarios

REPO_ROOT = Path(__file__).resolve().parents[2]
SCENARIOS_PATH = REPO_ROOT / "benchmarks" / "scenarios-v0.3.0.toml"
SUITE_PATH = REPO_ROOT / "benchmarks" / "competitors-v0.3.0.toml"
CORPUS_PATH = REPO_ROOT / "benchmarks" / "corpus-v0.3.0.toml"
POLICY_PATH = REPO_ROOT / "benchmarks" / "equivalence-v0.3.0.toml"
REGISTRY_PATH = REPO_ROOT / "compat" / "fixture-provenance.toml"
SCRIPT_PATH = REPO_ROOT / "scripts" / "run_benchmark_scenarios.py"
REPORT_PATH = REPO_ROOT / "docs" / "benchmarks" / "scenarios-v0.3.0.md"

REQUIRED_SCENARIOS = (
    "cold-document-open",
    "warm-document-open",
    "cache-hit-characters",
    "single-page-text",
    "full-document-text",
    "parallel-page-batch-text",
)


class BenchmarkScenarioContractTests(unittest.TestCase):
    def load_suite(self) -> benchmark_scenarios.ScenarioSuite:
        return benchmark_scenarios.audit_repository(
            REPO_ROOT,
            SCENARIOS_PATH,
            SUITE_PATH,
            CORPUS_PATH,
            POLICY_PATH,
            REGISTRY_PATH,
        )

    def test_repository_plan_distinguishes_every_required_workload(self) -> None:
        suite = self.load_suite()

        self.assertEqual(
            tuple(scenario.id for scenario in suite.scenarios),
            REQUIRED_SCENARIOS,
        )
        self.assertEqual(suite.filesystem_cache_control, "uncontrolled-recorded")

        cold = suite.scenario("cold-document-open")
        warm = suite.scenario("warm-document-open")
        cache_hit = suite.scenario("cache-hit-characters")
        single_page = suite.scenario("single-page-text")
        full_document = suite.scenario("full-document-text")
        parallel = suite.scenario("parallel-page-batch-text")
        assert all(
            scenario is not None
            for scenario in (
                cold,
                warm,
                cache_hit,
                single_page,
                full_document,
                parallel,
            )
        )
        assert cold is not None
        assert warm is not None
        assert cache_hit is not None
        assert single_page is not None
        assert full_document is not None
        assert parallel is not None

        self.assertEqual(cold.process_state, "fresh-adapter-process")
        self.assertEqual(cold.cache_state, "library-state-empty")
        self.assertEqual(cold.setup_operations, ("fixture-digest-verified",))
        self.assertEqual(warm.process_state, "reused-adapter-process")
        self.assertEqual(warm.cache_state, "prior-identical-open-completed")
        self.assertIn("untimed-identical-open-and-close", warm.setup_operations)
        self.assertEqual(cold.timed_operation, warm.timed_operation)

        self.assertEqual(cache_hit.cache_state, "first-identical-access-completed")
        self.assertIn("same-live-page", cache_hit.setup_operations)
        self.assertEqual(cache_hit.timed_operation, "second-page-chars-access")
        self.assertEqual(
            cache_hit.semantic_implementations,
            ("pdfplumber-python", "pdfplumber-rs-python"),
        )

        self.assertEqual(single_page.page_selection, "first-page")
        self.assertEqual(full_document.page_selection, "all-pages")
        self.assertEqual(single_page.fixture_ids, full_document.fixture_ids)
        for scenario in (single_page, full_document):
            with self.subTest(fused_implementation=scenario.id):
                self.assertIn("pdfsink-rs", scenario.semantic_implementations)
                self.assertNotIn("pdfsink-rs", scenario.timed_implementations)
        self.assertTrue(
            all(
                suite.competitor_suite.corpus.fixture(fixture_id).page_count > 1
                for fixture_id in single_page.fixture_ids
            )
        )

        self.assertEqual(parallel.concurrency_model, "bounded-rayon-thread-pool")
        self.assertEqual(parallel.worker_count, 4)
        self.assertEqual(parallel.output_order, "page-index-order")
        self.assertEqual(parallel.timed_implementations, ("pdfplumber-rs",))
        self.assertEqual(parallel.page_selection, "all-pages")

    def test_timing_sample_requires_exact_untimed_scenario_output(self) -> None:
        scenario = self.load_suite().scenario("single-page-text")
        assert scenario is not None
        untimed = benchmark_scenarios.synthetic_scenario_record(
            scenario=scenario,
            implementation_id="pdfplumber-rs",
            revision="a" * 40,
            fixtures=(
                (
                    "large-multipage",
                    "e9823b5527fed648c9b4a891b0c018d75a4acfa3697f9bfa91b2f8e5097579e0",
                ),
            ),
            outcome={
                "status": "success",
                "value": [{"page_number": 1, "text": "same"}],
            },
        )
        sample = benchmark_scenarios.build_scenario_sample(
            scenario=scenario,
            untimed_record=untimed,
            measured_outcome={
                "status": "success",
                "value": [{"page_number": 1, "text": "same"}],
            },
            wall_time_ns=123,
            command_argv=["adapter", "--scenario", scenario.id, "--timed"],
        )

        self.assertEqual(sample["scenario_id"], "single-page-text")
        self.assertEqual(sample["page_selection"], "first-page")
        self.assertEqual(sample["wall_time_ns"], 123)
        self.assertEqual(sample["clock"], "monotonic-wall")
        self.assertNotIn("process_launch", sample["timed_operation"])

        with self.assertRaisesRegex(
            benchmark_scenarios.BenchmarkScenarioError,
            "scenario output drifted",
        ):
            benchmark_scenarios.build_scenario_sample(
                scenario=scenario,
                untimed_record=untimed,
                measured_outcome={
                    "status": "success",
                    "value": [{"page_number": 1, "text": "different"}],
                },
                wall_time_ns=123,
                command_argv=["adapter", "--scenario", scenario.id, "--timed"],
            )

    def test_saved_run_keeps_rejections_separate_from_scenario_samples(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            output_path = Path(temporary_directory) / "scenario-run.json"
            benchmark_scenarios.write_local_run(
                output_path,
                records=[{"scenario_id": "full-document-text"}],
                preflight_decisions=[
                    {
                        "scenario_id": "full-document-text",
                        "implementation_id": "pdf-oxide",
                        "eligible_for_timing": False,
                        "reasons": ["candidate output differs from reference"],
                    }
                ],
                scenario_timings=[],
            )

            saved = json.loads(output_path.read_text(encoding="utf-8"))
            self.assertEqual(saved["publication_status"], "local-unpublished")
            self.assertFalse(
                saved["preflight_decisions"][0]["eligible_for_timing"]
            )
            self.assertEqual(saved["scenario_timings"], [])
            self.assertNotIn("statistical_summaries", saved)

    def test_cli_check_gates_manifest_report_adapters_ci_and_public_links(
        self,
    ) -> None:
        suite = self.load_suite()
        completed = subprocess.run(
            [sys.executable, str(SCRIPT_PATH), "--check"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("6 workload scenarios", completed.stdout)
        self.assertEqual(
            REPORT_PATH.read_text(encoding="utf-8"),
            benchmark_scenarios.render_markdown(suite),
        )
        workflow = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("python scripts/run_benchmark_scenarios.py --check", workflow)
        for adapter in (
            REPO_ROOT / "benchmarks" / "adapters" / "python_pdfplumber.py",
            REPO_ROOT / "benchmarks" / "adapters" / "python_pdfplumber_rs.py",
            REPO_ROOT / "benchmarks" / "adapters" / "rust" / "src" / "main.rs",
        ):
            with self.subTest(adapter=adapter):
                self.assertIn("--scenario", adapter.read_text(encoding="utf-8"))
        self.assertIn(
            "docs/benchmarks/scenarios-v0.3.0.md",
            (REPO_ROOT / "README.md").read_text(encoding="utf-8"),
        )
        self.assertIn(
            "benchmarks/scenarios-v0.3.0.md",
            (REPO_ROOT / "docs" / "comparison.md").read_text(encoding="utf-8"),
        )
        roadmap = (REPO_ROOT / "ROADMAP.md").read_text(encoding="utf-8")
        self.assertIn("`SCORE-006`", roadmap)
        self.assertIn("`SCORE-007`", roadmap)
        self.assertIn("`SCORE-008`", roadmap)
        self.assertIn("`SCORE-008`, and `SCORE-009`", roadmap)
        self.assertNotIn("Detailed task: [`SCORE-009`]", roadmap)


if __name__ == "__main__":
    unittest.main()
