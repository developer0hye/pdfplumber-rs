"""Resource and artifact benchmark contracts (SCORE-005)."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from compat.harness import benchmark_metrics, benchmark_stages

REPO_ROOT = Path(__file__).resolve().parents[2]
METRICS_PATH = REPO_ROOT / "benchmarks" / "metrics-v0.3.0.toml"
STAGES_PATH = REPO_ROOT / "benchmarks" / "stages-v0.3.0.toml"
SUITE_PATH = REPO_ROOT / "benchmarks" / "competitors-v0.3.0.toml"
CORPUS_PATH = REPO_ROOT / "benchmarks" / "corpus-v0.3.0.toml"
POLICY_PATH = REPO_ROOT / "benchmarks" / "equivalence-v0.3.0.toml"
REGISTRY_PATH = REPO_ROOT / "compat" / "fixture-provenance.toml"
SCRIPT_PATH = REPO_ROOT / "scripts" / "run_benchmark_metrics.py"
REPORT_PATH = REPO_ROOT / "docs" / "benchmarks" / "metrics-v0.3.0.md"


class BenchmarkMetricContractTests(unittest.TestCase):
    def load_stage_suite(self) -> benchmark_stages.StageSuite:
        return benchmark_stages.audit_repository(
            REPO_ROOT,
            STAGES_PATH,
            SUITE_PATH,
            CORPUS_PATH,
            POLICY_PATH,
            REGISTRY_PATH,
        )

    def load_metric_suite(self) -> benchmark_metrics.MetricSuite:
        return benchmark_metrics.audit_repository(
            REPO_ROOT,
            METRICS_PATH,
            self.load_stage_suite(),
        )

    def test_repository_plan_separates_wall_and_resource_passes(self) -> None:
        suite = self.load_metric_suite()

        self.assertEqual(suite.stage_suite_id, "pdfplumber-rs-stages-v0.3.0")
        self.assertEqual(suite.wall_measurement_pass, "un-instrumented")
        self.assertEqual(suite.resource_measurement_pass, "separate-instrumented")
        self.assertEqual(suite.resource_platforms, ("linux", "macos"))
        self.assertEqual(suite.cpu_scope, "in-adapter-stage-only")
        self.assertEqual(
            suite.peak_rss_scope,
            "adapter-process-lifetime-high-water",
        )
        self.assertEqual(
            {allocator.runtime for allocator in suite.allocators},
            {"python", "rust"},
        )
        python = suite.allocator("python")
        rust = suite.allocator("rust")
        assert python is not None and rust is not None
        self.assertEqual(python.method, "python-tracemalloc")
        self.assertIn("retained_allocation_count", python.metrics)
        self.assertIn("peak_traced_bytes", python.metrics)
        self.assertEqual(rust.method, "rust-counting-global-allocator")
        self.assertIn("gross_allocation_count", rust.metrics)
        self.assertIn("gross_allocated_bytes", rust.metrics)
        self.assertNotEqual(python.method, rust.method)

    def test_resource_sample_is_exact_output_gated_and_scope_explicit(self) -> None:
        stage = self.load_stage_suite().stage("serialization")
        assert stage is not None
        untimed = benchmark_stages.synthetic_stage_record(
            stage=stage,
            implementation_id="pdfplumber-rs",
            revision="b" * 40,
            fixture_id="small-text",
            fixture_sha256=(
                "22b6f9bd4aa388d7e6fb116d45bbe15e6da84c8d23fe20582d857e4b05c809ec"
            ),
            outcome={"status": "success", "value": {"utf8_bytes": 37}},
        )
        resource = {
            "stage_id": "serialization",
            "cpu": {
                "clock": "process-cpu",
                "scope": "in-adapter-stage-only",
                "time_ns": 321,
            },
            "peak_resident_memory": {
                "scope": "adapter-process-lifetime-high-water",
                "bytes": 4096,
            },
            "allocations": {
                "method": "rust-counting-global-allocator",
                "scope": "in-adapter-stage-only",
                "gross_allocation_count": 4,
                "gross_allocated_bytes": 128,
            },
        }
        sample = benchmark_metrics.build_resource_sample(
            stage=stage,
            untimed_record=untimed,
            measured_outcome={"status": "success", "value": {"utf8_bytes": 37}},
            resource=resource,
            command_argv=["adapter", "--stage", "serialization", "--resources"],
        )

        self.assertEqual(sample["cpu_time_ns"], 321)
        self.assertEqual(sample["peak_resident_memory_bytes"], 4096)
        self.assertEqual(
            sample["peak_resident_memory_scope"],
            "adapter-process-lifetime-high-water",
        )
        self.assertEqual(
            sample["allocations"]["method"],
            "rust-counting-global-allocator",
        )
        self.assertNotIn("wall_time_ns", sample)

        with self.assertRaisesRegex(
            benchmark_metrics.BenchmarkMetricError,
            "resource output drifted",
        ):
            benchmark_metrics.build_resource_sample(
                stage=stage,
                untimed_record=untimed,
                measured_outcome={"status": "success", "value": {"utf8_bytes": 38}},
                resource=resource,
                command_argv=["adapter", "--resources"],
            )

    def test_artifacts_are_candidate_attributable_and_wasm_startup_is_internal(
        self,
    ) -> None:
        suite = self.load_metric_suite()
        artifacts = {artifact.id: artifact for artifact in suite.artifacts}

        self.assertEqual(set(artifacts), {"native-cli", "wasm-node-package"})
        self.assertEqual(artifacts["native-cli"].kind, "native-executable")
        self.assertEqual(
            artifacts["native-cli"].paths,
            ("target/release/pdfplumber",),
        )
        wasm = artifacts["wasm-node-package"]
        self.assertEqual(wasm.kind, "wasm-package")
        wasm_names = {Path(path).name for path in wasm.paths}
        self.assertIn("pdfplumber_wasm_bg.wasm", wasm_names)
        self.assertIn("pdfplumber_wasm.js", wasm_names)
        self.assertNotIn("benchmarks/adapters/rust/target", " ".join(wasm.paths))
        assert suite.wasm_startup is not None
        self.assertEqual(suite.wasm_startup.process_model, "fresh-process-per-sample")
        self.assertEqual(suite.wasm_startup.clock_scope, "node-module-load-only")
        self.assertFalse(suite.wasm_startup.includes_process_launch)

    def test_saved_run_keeps_wall_resources_sizes_and_startup_separate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            output_path = Path(temporary_directory) / "metrics.json"
            benchmark_metrics.write_local_run(
                output_path,
                records=[{"case": "small-text:serialization"}],
                preflight_decisions=[{"eligible_for_timing": True}],
                stage_timings=[{"wall_time_ns": 10}],
                stage_resources=[{"cpu_time_ns": 8}],
                artifact_sizes=[{"artifact_id": "native-cli", "bytes": 1024}],
                wasm_startup=[{"wall_time_ns": 99}],
            )

            saved = json.loads(output_path.read_text(encoding="utf-8"))
            self.assertEqual(saved["publication_status"], "local-unpublished")
            self.assertEqual(saved["stage_timings"], [{"wall_time_ns": 10}])
            self.assertEqual(saved["stage_resources"], [{"cpu_time_ns": 8}])
            self.assertEqual(saved["artifact_sizes"][0]["bytes"], 1024)
            self.assertEqual(saved["wasm_startup"][0]["wall_time_ns"], 99)

    def test_cli_check_gates_manifest_report_adapters_ci_and_public_claims(
        self,
    ) -> None:
        suite = self.load_metric_suite()
        completed = subprocess.run(
            [sys.executable, str(SCRIPT_PATH), "--check"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("resource and artifact metrics", completed.stdout)
        self.assertEqual(
            REPORT_PATH.read_text(encoding="utf-8"),
            benchmark_metrics.render_markdown(suite),
        )
        workflow = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("python scripts/run_benchmark_metrics.py --check", workflow)
        for adapter in (
            REPO_ROOT / "benchmarks" / "adapters" / "python_pdfplumber.py",
            REPO_ROOT / "benchmarks" / "adapters" / "python_pdfplumber_rs.py",
            REPO_ROOT / "benchmarks" / "adapters" / "rust" / "src" / "main.rs",
        ):
            with self.subTest(adapter=adapter):
                self.assertIn("--resources", adapter.read_text(encoding="utf-8"))
        self.assertIn(
            "docs/benchmarks/metrics-v0.3.0.md",
            (REPO_ROOT / "README.md").read_text(encoding="utf-8"),
        )
        self.assertIn(
            "benchmarks/metrics-v0.3.0.md",
            (REPO_ROOT / "docs" / "comparison.md").read_text(encoding="utf-8"),
        )


if __name__ == "__main__":
    unittest.main()
