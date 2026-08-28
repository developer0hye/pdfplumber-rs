"""Benchmark-regression alert and noise-policy contracts (SCORE-013)."""

from __future__ import annotations

import copy
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from compat.harness import benchmark_provenance, benchmark_regressions
from compat.tests.test_benchmark_results import local_run

REPO_ROOT = Path(__file__).resolve().parents[2]
REGRESSION_PATH = REPO_ROOT / "benchmarks" / "regressions-v0.3.0.toml"
RETENTION_PATH = REPO_ROOT / "benchmarks" / "result-retention-v0.3.0.toml"
PUBLICATION_PATH = REPO_ROOT / "benchmarks" / "results-v0.3.0.toml"
PROVENANCE_PATH = REPO_ROOT / "benchmarks" / "provenance-v0.3.0.toml"
SCENARIOS_PATH = REPO_ROOT / "benchmarks" / "scenarios-v0.3.0.toml"
SUITE_PATH = REPO_ROOT / "benchmarks" / "competitors-v0.3.0.toml"
CORPUS_PATH = REPO_ROOT / "benchmarks" / "corpus-v0.3.0.toml"
EQUIVALENCE_PATH = REPO_ROOT / "benchmarks" / "equivalence-v0.3.0.toml"
REGISTRY_PATH = REPO_ROOT / "compat" / "fixture-provenance.toml"
DOCUMENT_PATH = REPO_ROOT / "docs" / "benchmarks" / "regressions-v0.3.0.md"
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "benchmark-regressions.yml"
SCRIPT_PATH = REPO_ROOT / "scripts" / "check_benchmark_regressions.py"

BASELINE_REVISION = "a" * 40
CANDIDATE_REVISION = "b" * 40
CONTROL_REVISION = "c" * 40


def repository_policy() -> benchmark_regressions.RegressionPolicy:
    return benchmark_regressions.audit_repository(
        REPO_ROOT,
        REGRESSION_PATH,
        RETENTION_PATH,
        PUBLICATION_PATH,
        PROVENANCE_PATH,
        SCENARIOS_PATH,
        SUITE_PATH,
        CORPUS_PATH,
        EQUIVALENCE_PATH,
        REGISTRY_PATH,
    )


def test_policy() -> benchmark_regressions.RegressionPolicy:
    return benchmark_regressions.RegressionPolicy(
        schema_version=1,
        id="test-regression-policy",
        release="0.3.0",
        baseline_tag="benchmark-results-v0.3.0",
        runner="macos-14",
        run_order=("baseline", "candidate", "candidate", "baseline"),
        runs_per_revision=2,
        repetitions_per_run=5,
        target_implementations=("pdfplumber-rs",),
        control_implementations=("pdfplumber-python",),
        minimum_control_groups=1,
        minimum_slowdown_fraction=0.20,
        noise_multiplier=3.0,
        lower_quantile=0.25,
        upper_quantile=0.75,
    )


def benchmark_run(
    *,
    source_revision: str,
    control_times: tuple[int, ...],
    target_times: tuple[int, ...],
    target_digest: str = "c" * 64,
    target_is_eligible: bool = True,
) -> dict[str, object]:
    run = copy.deepcopy(local_run())
    run["run_metadata"]["source"]["revision"] = source_revision
    timings = run["scenario_timings"]
    for timing in timings:
        implementation = timing["implementation"]
        repetition = timing["repetition"]
        if implementation["id"] == "pdfplumber-python":
            implementation["revision"] = CONTROL_REVISION
            timing["wall_time_ns"] = control_times[repetition - 1]
        elif implementation["id"] == "pdfplumber-rs":
            implementation["revision"] = source_revision
            timing["wall_time_ns"] = target_times[repetition - 1]
            timing["semantic_output_sha256"] = target_digest
    decision = run["preflight_decisions"][0]
    decision["eligible_for_timing"] = target_is_eligible
    decision["candidate_output_sha256"] = target_digest
    decision["reasons"] = (
        [] if target_is_eligible else ["candidate output differs from reference"]
    )
    run["statistical_summaries"] = benchmark_provenance.summarize_samples(
        timings,
        repetitions=5,
    )
    return run


def paired_runs(
    *,
    control_times: tuple[int, ...],
    baseline_target_times: tuple[int, ...],
    candidate_target_times: tuple[int, ...],
) -> tuple[list[dict[str, object]], list[dict[str, object]]]:
    baselines = [
        benchmark_run(
            source_revision=BASELINE_REVISION,
            control_times=control_times,
            target_times=baseline_target_times,
        )
        for _ in range(2)
    ]
    candidates = [
        benchmark_run(
            source_revision=CANDIDATE_REVISION,
            control_times=control_times,
            target_times=candidate_target_times,
        )
        for _ in range(2)
    ]
    return baselines, candidates


class BenchmarkRegressionTests(unittest.TestCase):
    def test_repository_policy_documents_fixed_fail_closed_alerts(self) -> None:
        policy = repository_policy()

        self.assertEqual(policy.baseline_tag, "benchmark-results-v0.3.0")
        self.assertEqual(policy.runner, "macos-14")
        self.assertEqual(
            policy.run_order,
            ("baseline", "candidate", "candidate", "baseline"),
        )
        self.assertEqual(policy.runs_per_revision, 2)
        self.assertEqual(policy.repetitions_per_run, 5)
        self.assertEqual(policy.minimum_slowdown_fraction, 0.20)
        self.assertEqual(policy.noise_multiplier, 3.0)
        self.assertEqual(
            set(policy.target_implementations),
            {"pdfplumber-rs", "pdfplumber-rs-python"},
        )
        self.assertEqual(
            set(policy.control_implementations),
            {"pdfplumber-python", "pdf-oxide", "pdfsink-rs"},
        )
        self.assertEqual(
            DOCUMENT_PATH.read_text(encoding="utf-8"),
            benchmark_regressions.render_policy(policy),
        )

    def test_control_normalization_does_not_alert_on_shared_host_slowdown(self) -> None:
        baselines, candidates = paired_runs(
            control_times=(120, 121, 122, 123, 124),
            baseline_target_times=(50, 51, 52, 53, 54),
            candidate_target_times=(60, 61, 62, 63, 64),
        )
        for baseline in baselines:
            for timing in baseline["scenario_timings"]:
                if timing["implementation"]["id"] == "pdfplumber-python":
                    timing["wall_time_ns"] = round(timing["wall_time_ns"] / 1.2)
            baseline["statistical_summaries"] = benchmark_provenance.summarize_samples(
                baseline["scenario_timings"], repetitions=5
            )

        decision = benchmark_regressions.compare_runs(
            test_policy(), baselines, candidates
        )

        self.assertEqual(decision.status, "pass")
        self.assertAlmostEqual(decision.control_scale, 1.2, places=2)
        self.assertEqual(decision.comparisons[0].status, "within-policy")

    def test_clear_low_noise_slowdown_emits_regression(self) -> None:
        baselines, candidates = paired_runs(
            control_times=(100, 101, 102, 103, 104),
            baseline_target_times=(50, 51, 52, 53, 54),
            candidate_target_times=(85, 86, 87, 88, 89),
        )

        decision = benchmark_regressions.compare_runs(
            test_policy(), baselines, candidates
        )

        self.assertEqual(decision.status, "regression")
        comparison = decision.comparisons[0]
        self.assertEqual(comparison.status, "regression")
        self.assertGreater(comparison.normalized_slowdown_fraction, 0.60)
        self.assertTrue(comparison.distributions_separated)

    def test_noisy_overlapping_samples_do_not_emit_false_regression(self) -> None:
        baselines, candidates = paired_runs(
            control_times=(100, 101, 102, 103, 104),
            baseline_target_times=(40, 45, 50, 55, 60),
            candidate_target_times=(20, 30, 65, 100, 150),
        )

        decision = benchmark_regressions.compare_runs(
            test_policy(), baselines, candidates
        )

        self.assertEqual(decision.status, "pass")
        self.assertEqual(decision.comparisons[0].status, "noise-overlap")
        self.assertFalse(decision.comparisons[0].distributions_separated)

    def test_semantic_drift_fails_before_performance_thresholds(self) -> None:
        baselines, candidates = paired_runs(
            control_times=(100, 101, 102, 103, 104),
            baseline_target_times=(50, 51, 52, 53, 54),
            candidate_target_times=(10, 11, 12, 13, 14),
        )
        for candidate in candidates:
            decision = candidate["preflight_decisions"][0]
            decision["eligible_for_timing"] = False
            decision["candidate_output_sha256"] = "f" * 64
            decision["reasons"] = ["candidate output differs from reference"]
            for timing in candidate["scenario_timings"]:
                if timing["implementation"]["id"] == "pdfplumber-rs":
                    timing["semantic_output_sha256"] = "f" * 64
            candidate["statistical_summaries"] = benchmark_provenance.summarize_samples(
                candidate["scenario_timings"], repetitions=5
            )

        decision = benchmark_regressions.compare_runs(
            test_policy(), baselines, candidates
        )

        self.assertEqual(decision.status, "semantic-failure")
        self.assertEqual(decision.comparisons, ())
        self.assertTrue(
            any("output-equivalence" in reason for reason in decision.reasons)
        )

    def test_cli_and_read_only_workflow_run_abba_and_retain_decision(self) -> None:
        completed = subprocess.run(
            [sys.executable, str(SCRIPT_PATH), "--check"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("Benchmark regression policy OK", completed.stdout)

        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        for required in (
            "schedule:",
            "workflow_dispatch:",
            "permissions:\n  contents: read",
            "path: baseline",
            "path: candidate",
            "baseline-a.json",
            "candidate-a.json",
            "candidate-b.json",
            "baseline-b.json",
            "check_benchmark_regressions.py --compare",
            "benchmark-regression-decision.json",
        ):
            self.assertIn(required, workflow)
        self.assertLess(
            workflow.index("baseline-a.json"), workflow.index("candidate-a.json")
        )
        self.assertLess(
            workflow.index("candidate-a.json"), workflow.index("candidate-b.json")
        )
        self.assertLess(
            workflow.index("candidate-b.json"), workflow.index("baseline-b.json")
        )
        for prohibited in ("contents: write", "issues: write", "pull-requests: write"):
            self.assertNotIn(prohibited, workflow)

        ci = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("python scripts/check_benchmark_regressions.py --check", ci)

    def test_incomplete_run_still_retains_an_inconclusive_decision(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            directory = Path(temporary_directory)
            decision_path = directory / "decision.json"
            missing_path = directory / "missing.json"
            completed = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT_PATH),
                    "--compare",
                    "--baseline-run",
                    str(missing_path),
                    "--baseline-run",
                    str(missing_path),
                    "--candidate-run",
                    str(missing_path),
                    "--candidate-run",
                    str(missing_path),
                    "--decision-output",
                    str(decision_path),
                ],
                cwd=REPO_ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
            decision = json.loads(decision_path.read_text(encoding="utf-8"))

        self.assertEqual(completed.returncode, 1, completed.stderr)
        self.assertEqual(decision["status"], "inconclusive")
        self.assertTrue(
            any("cannot read benchmark run" in reason for reason in decision["reasons"])
        )


if __name__ == "__main__":
    unittest.main()
