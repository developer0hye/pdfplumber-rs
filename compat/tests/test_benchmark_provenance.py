"""Reproducible benchmark run metadata and statistics contracts (SCORE-007)."""

from __future__ import annotations

import json
import math
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from compat.harness import benchmark_provenance

REPO_ROOT = Path(__file__).resolve().parents[2]
PROVENANCE_PATH = REPO_ROOT / "benchmarks" / "provenance-v0.3.0.toml"
SCENARIOS_PATH = REPO_ROOT / "benchmarks" / "scenarios-v0.3.0.toml"
SUITE_PATH = REPO_ROOT / "benchmarks" / "competitors-v0.3.0.toml"
CORPUS_PATH = REPO_ROOT / "benchmarks" / "corpus-v0.3.0.toml"
POLICY_PATH = REPO_ROOT / "benchmarks" / "equivalence-v0.3.0.toml"
REGISTRY_PATH = REPO_ROOT / "compat" / "fixture-provenance.toml"
SCRIPT_PATH = REPO_ROOT / "scripts" / "run_benchmark_provenance.py"
REPORT_PATH = REPO_ROOT / "docs" / "benchmarks" / "provenance-v0.3.0.md"


def sample(repetition: int, wall_time_ns: int) -> dict[str, object]:
    return {
        "schema_version": 1,
        "case_id": "large-multipage:full-document-text",
        "scenario_id": "full-document-text",
        "implementation": {"id": "pdfplumber-rs", "revision": "a" * 40},
        "fixtures": [{"id": "large-multipage", "sha256": "b" * 64}],
        "measurement_scope": "in-adapter-scenario-only",
        "process_state": "fresh-adapter-process",
        "cache_state": "document-open-page-cache-empty",
        "page_selection": "all-pages",
        "concurrency_model": "serial",
        "worker_count": 1,
        "output_order": "page-index-order",
        "clock": "monotonic-wall",
        "setup_operations": ["fixture-digest-verified", "document-open"],
        "timed_operation": "extract-all-pages-plain-text",
        "wall_time_ns": wall_time_ns,
        "semantic_output_sha256": "c" * 64,
        "command_argv": [
            "benchmarks/adapters/rust/target/release/pdf-benchmark-adapter",
            "pdfplumber-rs",
            "--scenario",
            "full-document-text",
            "--fixture",
            "compat/fixtures/benchmark/large-multipage.pdf",
            "--timed",
        ],
        "repetition": repetition,
    }


def run_metadata() -> dict[str, object]:
    return {
        "recorded_at_utc": "2026-08-28T00:00:00Z",
        "source": {
            "repository": "https://github.com/developer0hye/pdfplumber-rs",
            "revision": "a" * 40,
            "working_tree_clean": True,
        },
        "host": {
            "operating_system": "Darwin",
            "operating_system_release": "25.6.0",
            "architecture": "arm64",
            "cpu_model": "Apple M4 Max",
            "logical_cpu_count": 16,
            "physical_memory_bytes": 64 * 1024**3,
        },
        "toolchains": [
            {
                "id": tool_id,
                "command_argv": [tool_id, "--version"],
                "version": f"{tool_id} test-version",
            }
            for tool_id in (
                "harness-python",
                "reference-python",
                "candidate-python",
                "rustc",
                "cargo",
                "maturin",
            )
        ],
        "builds": [
            {
                "id": "rust-benchmark-adapter",
                "command_argv": [
                    "cargo",
                    "build",
                    "--manifest-path",
                    "benchmarks/adapters/rust/Cargo.toml",
                    "--release",
                    "--locked",
                ],
                "flags": ["--release", "--locked", "features=parallel"],
            },
            {
                "id": "candidate-python-wheel",
                "command_argv": [
                    "python3.13",
                    "scripts/setup_candidate_venv.py",
                    "--python",
                    "python3.13",
                ],
                "flags": ["maturin=1.14.1", "pip=--no-deps"],
            },
        ],
        "dependency_locks": [
            {"role": role, "path": path, "sha256": "d" * 64}
            for role, path in (
                ("rust-workspace", "Cargo.lock"),
                ("rust-benchmark-adapter", "benchmarks/adapters/rust/Cargo.lock"),
                ("python-reference", "compat/requirements-golden.txt"),
            )
        ],
        "artifacts": [
            {
                "role": "rust-benchmark-adapter",
                "path": "benchmarks/adapters/rust/target/release/pdf-benchmark-adapter",
                "sha256": "e" * 64,
                "size_bytes": 123,
            },
            {
                "role": "candidate-python-native-extension",
                "path": ".venv-candidate/lib/python3.13/site-packages/pdfplumber/_native.so",
                "sha256": "f" * 64,
                "size_bytes": 456,
            },
        ],
        "fixtures": [{"id": "large-multipage", "sha256": "b" * 64}],
        "invocation": {
            "working_directory": ".",
            "command_argv": [
                "python3",
                "scripts/run_benchmark_provenance.py",
                "--run",
                "--output",
                "<output-json>",
            ],
            "repetitions": 5,
            "execution_order": "round-robin-by-repetition",
        },
    }


class BenchmarkProvenanceContractTests(unittest.TestCase):
    def load_plan(self) -> benchmark_provenance.ProvenancePlan:
        return benchmark_provenance.audit_repository(
            REPO_ROOT,
            PROVENANCE_PATH,
            SCENARIOS_PATH,
            SUITE_PATH,
            CORPUS_PATH,
            POLICY_PATH,
            REGISTRY_PATH,
        )

    def test_repository_plan_records_every_reproduction_input(self) -> None:
        plan = self.load_plan()

        self.assertEqual(plan.repetitions, 5)
        self.assertEqual(plan.execution_order, "round-robin-by-repetition")
        self.assertEqual(plan.working_tree_policy, "require-clean")
        self.assertEqual(
            plan.statistics,
            (
                "sample-size",
                "minimum",
                "median",
                "arithmetic-mean",
                "maximum",
                "sample-standard-deviation",
                "relative-standard-deviation",
            ),
        )
        self.assertEqual(
            {lock.role for lock in plan.dependency_locks},
            {"rust-workspace", "rust-benchmark-adapter", "python-reference"},
        )
        self.assertEqual(
            {build.id for build in plan.builds},
            {"rust-benchmark-adapter", "candidate-python-wheel"},
        )
        rust_build = next(
            build for build in plan.builds if build.id == "rust-benchmark-adapter"
        )
        self.assertIn("--release", rust_build.flags)
        self.assertIn("--locked", rust_build.flags)
        self.assertIn("features=parallel", rust_build.flags)
        candidate_build = next(
            build for build in plan.builds if build.id == "candidate-python-wheel"
        )
        self.assertIn("maturin=1.14.1", candidate_build.flags)
        self.assertIn("pip=--no-deps", candidate_build.flags)
        self.assertIn("profile=release", candidate_build.flags)
        self.assertEqual(
            {tool.id for tool in plan.tools},
            {
                "harness-python",
                "reference-python",
                "candidate-python",
                "rustc",
                "cargo",
                "maturin",
            },
        )
        self.assertEqual(plan.scenario_suite.id, "pdfplumber-rs-scenarios-v0.3.0")

    def test_five_raw_repetitions_produce_one_bound_statistical_summary(self) -> None:
        samples = [
            sample(repetition, wall_time_ns)
            for repetition, wall_time_ns in enumerate(
                (100, 200, 300, 400, 500), start=1
            )
        ]

        summaries = benchmark_provenance.summarize_samples(samples, repetitions=5)

        self.assertEqual(len(summaries), 1)
        summary = summaries[0]
        self.assertEqual(summary["sample_size"], 5)
        self.assertEqual(summary["repetitions"], [1, 2, 3, 4, 5])
        self.assertEqual(summary["minimum_wall_time_ns"], 100)
        self.assertEqual(summary["median_wall_time_ns"], 300)
        self.assertEqual(summary["arithmetic_mean_wall_time_ns"], 300)
        self.assertEqual(summary["maximum_wall_time_ns"], 500)
        self.assertTrue(
            math.isclose(
                summary["sample_standard_deviation_wall_time_ns"],
                math.sqrt(25_000),
            )
        )
        self.assertTrue(
            math.isclose(
                summary["relative_standard_deviation"],
                math.sqrt(25_000) / 300,
            )
        )
        self.assertEqual(summary["semantic_output_sha256"], "c" * 64)
        self.assertRegex(str(summary["samples_sha256"]), r"^[0-9a-f]{64}$")

        with self.assertRaisesRegex(
            benchmark_provenance.BenchmarkProvenanceError,
            "repetitions",
        ):
            benchmark_provenance.summarize_samples(samples[:-1], repetitions=5)

    def test_saved_run_requires_complete_metadata_and_retains_raw_samples(self) -> None:
        samples = [
            sample(repetition, wall_time_ns)
            for repetition, wall_time_ns in enumerate(
                (100, 200, 300, 400, 500), start=1
            )
        ]
        summaries = benchmark_provenance.summarize_samples(samples, repetitions=5)
        metadata = run_metadata()

        with tempfile.TemporaryDirectory() as temporary_directory:
            output_path = Path(temporary_directory) / "run.json"
            benchmark_provenance.write_local_run(
                output_path,
                run_metadata=metadata,
                records=[{"scenario_id": "full-document-text"}],
                preflight_decisions=[
                    {
                        "scenario_id": "full-document-text",
                        "implementation_id": "pdf-oxide",
                        "eligible_for_timing": False,
                        "reasons": ["candidate output differs from reference"],
                    }
                ],
                scenario_timings=samples,
                statistical_summaries=summaries,
            )

            saved = json.loads(output_path.read_text(encoding="utf-8"))
            self.assertEqual(saved["publication_status"], "local-unpublished")
            self.assertEqual(saved["run_metadata"], metadata)
            self.assertEqual(len(saved["scenario_timings"]), 5)
            self.assertEqual(len(saved["statistical_summaries"]), 1)
            self.assertFalse(saved["preflight_decisions"][0]["eligible_for_timing"])

        incomplete = run_metadata()
        host = incomplete["host"]
        assert isinstance(host, dict)
        host.pop("cpu_model")
        with self.assertRaisesRegex(
            benchmark_provenance.BenchmarkProvenanceError,
            "cpu_model",
        ):
            benchmark_provenance.validate_run_metadata(incomplete, repetitions=5)

    def test_cli_check_gates_report_ci_and_public_links(self) -> None:
        plan = self.load_plan()
        completed = subprocess.run(
            [sys.executable, str(SCRIPT_PATH), "--check"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("5 repetitions", completed.stdout)
        self.assertEqual(
            REPORT_PATH.read_text(encoding="utf-8"),
            benchmark_provenance.render_markdown(plan),
        )
        workflow = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("python scripts/run_benchmark_provenance.py --check", workflow)
        self.assertIn(
            "docs/benchmarks/provenance-v0.3.0.md",
            (REPO_ROOT / "README.md").read_text(encoding="utf-8"),
        )
        self.assertIn(
            "benchmarks/provenance-v0.3.0.md",
            (REPO_ROOT / "docs" / "comparison.md").read_text(encoding="utf-8"),
        )
        roadmap = (REPO_ROOT / "ROADMAP.md").read_text(encoding="utf-8")
        self.assertIn("`SCORE-007`", roadmap)
        self.assertIn("[`SCORE-008`]", roadmap)


if __name__ == "__main__":
    unittest.main()
