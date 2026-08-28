"""Retained benchmark release-artifact contracts (SCORE-008)."""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from compat.harness import benchmark_provenance, benchmark_results

REPO_ROOT = Path(__file__).resolve().parents[2]
PUBLICATION_PATH = REPO_ROOT / "benchmarks" / "results-v0.3.0.toml"
PROVENANCE_PATH = REPO_ROOT / "benchmarks" / "provenance-v0.3.0.toml"
SCENARIOS_PATH = REPO_ROOT / "benchmarks" / "scenarios-v0.3.0.toml"
SUITE_PATH = REPO_ROOT / "benchmarks" / "competitors-v0.3.0.toml"
CORPUS_PATH = REPO_ROOT / "benchmarks" / "corpus-v0.3.0.toml"
POLICY_PATH = REPO_ROOT / "benchmarks" / "equivalence-v0.3.0.toml"
REGISTRY_PATH = REPO_ROOT / "compat" / "fixture-provenance.toml"
SCRIPT_PATH = REPO_ROOT / "scripts" / "publish_benchmark_results.py"
INDEX_PATH = REPO_ROOT / "docs" / "benchmarks" / "results-v0.3.0.md"
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "benchmark-results.yml"
SOURCE_REVISION = "a" * 40


def sample(
    implementation_id: str,
    repetition: int,
    wall_time_ns: int,
) -> dict[str, object]:
    return {
        "schema_version": 1,
        "case_id": "large-multipage:full-document-text",
        "scenario_id": "full-document-text",
        "implementation": {"id": implementation_id, "revision": SOURCE_REVISION},
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
            implementation_id,
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
            "revision": SOURCE_REVISION,
            "working_tree_clean": True,
        },
        "host": {
            "operating_system": "Darwin",
            "operating_system_release": "25.6.0",
            "architecture": "arm64",
            "cpu_model": "Apple M2",
            "logical_cpu_count": 8,
            "physical_memory_bytes": 16 * 1024**3,
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
                "flags": ["maturin=1.14.1", "profile=release", "pip=--no-deps"],
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


def local_run() -> dict[str, object]:
    timings = [
        sample(implementation_id, repetition, base + repetition * 10)
        for implementation_id, base in (
            ("pdfplumber-python", 100),
            ("pdfplumber-rs", 50),
        )
        for repetition in range(1, 6)
    ]
    return {
        "schema_version": 1,
        "publication_status": "local-unpublished",
        "run_metadata": run_metadata(),
        "records": [
            {
                "scenario": {"id": "full-document-text"},
                "implementation": {"id": implementation_id},
                "outcome": {"status": status},
            }
            for implementation_id, status in (
                ("pdfplumber-python", "success"),
                ("pdfplumber-rs", "success"),
                ("pdf-oxide", "success"),
            )
        ],
        "preflight_decisions": [
            {
                "schema_version": 1,
                "case_id": "large-multipage:full-document-text",
                "scenario_id": "full-document-text",
                "implementation_id": "pdfplumber-rs",
                "reference_implementation": "pdfplumber-python",
                "eligible_for_timing": True,
                "reasons": [],
                "reference_output_sha256": "c" * 64,
                "candidate_output_sha256": "c" * 64,
            },
            {
                "schema_version": 1,
                "case_id": "large-multipage:full-document-text",
                "scenario_id": "full-document-text",
                "implementation_id": "pdf-oxide",
                "reference_implementation": "pdfplumber-python",
                "eligible_for_timing": False,
                "reasons": ["candidate output differs from reference"],
                "reference_output_sha256": "c" * 64,
                "candidate_output_sha256": "d" * 64,
            },
        ],
        "scenario_timings": timings,
        "statistical_summaries": benchmark_provenance.summarize_samples(
            timings,
            repetitions=5,
        ),
    }


class BenchmarkResultPublicationTests(unittest.TestCase):
    def load_plan(self) -> benchmark_results.PublicationPlan:
        return benchmark_results.audit_repository(
            REPO_ROOT,
            PUBLICATION_PATH,
            PROVENANCE_PATH,
            SCENARIOS_PATH,
            SUITE_PATH,
            CORPUS_PATH,
            POLICY_PATH,
            REGISTRY_PATH,
        )

    def test_publication_plan_uses_exact_versioned_release_assets(self) -> None:
        plan = self.load_plan()

        self.assertEqual(plan.release, "0.3.0")
        self.assertEqual(plan.release_tag, "benchmark-results-v0.3.0")
        self.assertEqual(plan.source_policy, "exact-tag-target")
        self.assertEqual(plan.runner, "macos-14")
        self.assertEqual(
            plan.raw_asset,
            "pdfplumber-rs-benchmark-results-v0.3.0.json",
        )
        self.assertEqual(
            plan.report_asset,
            "pdfplumber-rs-benchmark-results-v0.3.0.md",
        )
        self.assertEqual(
            plan.checksums_asset,
            "pdfplumber-rs-benchmark-results-v0.3.0.sha256",
        )
        for url in (plan.raw_url, plan.report_url, plan.checksums_url):
            self.assertIn(f"/releases/download/{plan.release_tag}/", url)

    def test_only_complete_exact_tag_run_can_be_promoted(self) -> None:
        plan = self.load_plan()
        published = benchmark_results.publish_run(
            plan,
            local_run(),
            release_tag=plan.release_tag,
            source_revision=SOURCE_REVISION,
        )

        self.assertEqual(published["publication_status"], "release-artifact")
        self.assertEqual(published["publication"]["release_tag"], plan.release_tag)
        self.assertEqual(len(published["scenario_timings"]), 10)
        self.assertEqual(len(published["statistical_summaries"]), 2)
        self.assertFalse(
            published["preflight_decisions"][1]["eligible_for_timing"]
        )

        wrong_tag = local_run()
        with self.assertRaisesRegex(
            benchmark_results.BenchmarkResultError,
            "release tag",
        ):
            benchmark_results.publish_run(
                plan,
                wrong_tag,
                release_tag="benchmark-results-v9.9.9",
                source_revision=SOURCE_REVISION,
            )

        timed_rejection = local_run()
        rejected_samples = [
            sample("pdf-oxide", repetition, 200 + repetition)
            for repetition in range(1, 6)
        ]
        timed_rejection["scenario_timings"].extend(rejected_samples)
        timed_rejection["statistical_summaries"] = (
            benchmark_provenance.summarize_samples(
                timed_rejection["scenario_timings"],
                repetitions=5,
            )
        )
        with self.assertRaisesRegex(
            benchmark_results.BenchmarkResultError,
            "rejected.*timed",
        ):
            benchmark_results.publish_run(
                plan,
                timed_rejection,
                release_tag=plan.release_tag,
                source_revision=SOURCE_REVISION,
            )

    def test_release_assets_retain_raw_data_and_render_descriptive_report(self) -> None:
        plan = self.load_plan()

        with tempfile.TemporaryDirectory() as temporary_directory:
            assets = benchmark_results.write_release_assets(
                plan,
                local_run(),
                Path(temporary_directory),
                release_tag=plan.release_tag,
                source_revision=SOURCE_REVISION,
            )
            published = json.loads(assets.raw_path.read_text(encoding="utf-8"))
            report = assets.report_path.read_text(encoding="utf-8")
            checksums = assets.checksums_path.read_text(encoding="utf-8")

        self.assertEqual(assets.raw_path.name, plan.raw_asset)
        self.assertEqual(assets.report_path.name, plan.report_asset)
        self.assertEqual(assets.checksums_path.name, plan.checksums_asset)
        self.assertEqual(published["scenario_timings"], local_run()["scenario_timings"])
        self.assertEqual(
            assets.raw_sha256,
            hashlib.sha256(assets.raw_bytes).hexdigest(),
        )
        self.assertIn(plan.raw_url, report)
        self.assertIn(assets.raw_sha256, report)
        self.assertIn("2 timed groups", report)
        self.assertIn("1 rejected comparison", report)
        self.assertIn("Relative standard deviation", report)
        self.assertNotRegex(report.lower(), r"\bwinner\b|\bfaster\b|\bspeedup\b")
        self.assertIn(f"{assets.raw_sha256}  {plan.raw_asset}", checksums)
        self.assertIn(f"{assets.report_sha256}  {plan.report_asset}", checksums)

    def test_cli_check_gates_workflow_index_ci_and_public_links(self) -> None:
        plan = self.load_plan()
        completed = subprocess.run(
            [sys.executable, str(SCRIPT_PATH), "--check"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn(plan.release_tag, completed.stdout)
        self.assertEqual(
            INDEX_PATH.read_text(encoding="utf-8"),
            benchmark_results.render_index(plan),
        )
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        for required in (
            "benchmark-results-v*",
            "runs-on: macos-14",
            "python-version: \"3.13.12\"",
            "toolchain: 1.98.0",
            "maturin==1.14.1",
            "run_benchmark_provenance.py --build",
            "run_benchmark_provenance.py --run",
            "publish_benchmark_results.py --build-assets",
            "softprops/action-gh-release@v2",
            plan.raw_asset,
            plan.report_asset,
            plan.checksums_asset,
        ):
            self.assertIn(required, workflow)
        ci = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("python scripts/publish_benchmark_results.py --check", ci)
        for path, link in (
            (REPO_ROOT / "README.md", "docs/benchmarks/results-v0.3.0.md"),
            (
                REPO_ROOT / "docs" / "comparison.md",
                "benchmarks/results-v0.3.0.md",
            ),
            (
                REPO_ROOT / "crates" / "pdfplumber-py" / "README.md",
                "../../docs/benchmarks/results-v0.3.0.md",
            ),
            (
                REPO_ROOT / "crates" / "pdfplumber-wasm" / "README.md",
                "../../docs/benchmarks/results-v0.3.0.md",
            ),
            (
                REPO_ROOT / "crates" / "pdfplumber" / "benches" / "README.md",
                "../../../docs/benchmarks/results-v0.3.0.md",
            ),
        ):
            self.assertIn(link, path.read_text(encoding="utf-8"))
        self.assertIn(plan.raw_url, INDEX_PATH.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
