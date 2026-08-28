"""Component-scoped benchmark timing contracts (SCORE-004)."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from compat.harness import benchmark_stages

REPO_ROOT = Path(__file__).resolve().parents[2]
STAGES_PATH = REPO_ROOT / "benchmarks" / "stages-v0.3.0.toml"
SUITE_PATH = REPO_ROOT / "benchmarks" / "competitors-v0.3.0.toml"
CORPUS_PATH = REPO_ROOT / "benchmarks" / "corpus-v0.3.0.toml"
POLICY_PATH = REPO_ROOT / "benchmarks" / "equivalence-v0.3.0.toml"
REGISTRY_PATH = REPO_ROOT / "compat" / "fixture-provenance.toml"
SCRIPT_PATH = REPO_ROOT / "scripts" / "run_stage_benchmarks.py"
REPORT_PATH = REPO_ROOT / "docs" / "benchmarks" / "stages-v0.3.0.md"

REQUIRED_STAGES = (
    "document-open",
    "page-materialization",
    "character-extraction",
    "word-grouping",
    "table-detection",
    "serialization",
    "language-boundary-conversion",
)


class BenchmarkStageContractTests(unittest.TestCase):
    def load_suite(self) -> benchmark_stages.StageSuite:
        return benchmark_stages.audit_repository(
            REPO_ROOT,
            STAGES_PATH,
            SUITE_PATH,
            CORPUS_PATH,
            POLICY_PATH,
            REGISTRY_PATH,
        )

    def test_repository_plan_separates_every_required_component(self) -> None:
        suite = self.load_suite()

        self.assertEqual(tuple(stage.id for stage in suite.stages), REQUIRED_STAGES)
        for stage in suite.stages:
            with self.subTest(stage=stage.id):
                self.assertTrue(stage.fixture_ids)
                self.assertTrue(stage.timed_implementations)
                self.assertTrue(stage.timed_operation)
                self.assertNotIn(stage.timed_operation, stage.setup_operations)
                self.assertNotIn("process-launch", stage.setup_operations)
                self.assertNotIn("process-launch", stage.timed_operation)
                self.assertEqual(stage.clock, "monotonic-wall")

        boundary = suite.stage("language-boundary-conversion")
        self.assertIsNotNone(boundary)
        assert boundary is not None
        self.assertEqual(boundary.semantic_reference, "pdfplumber-python")
        self.assertEqual(boundary.timed_implementations, ("pdfplumber-rs-python",))
        self.assertIn("native-page-cache-warm", boundary.setup_operations)
        self.assertEqual(boundary.timed_operation, "native-to-python-char-dicts")

    def test_timing_plan_requires_exact_untimed_stage_output(self) -> None:
        suite = self.load_suite()
        stage = suite.stage("character-extraction")
        assert stage is not None
        outcomes = {
            implementation_id: benchmark_stages.synthetic_stage_record(
                stage=stage,
                implementation_id=implementation_id,
                revision="a" * 40,
                fixture_id="small-text",
                fixture_sha256=(
                    "22b6f9bd4aa388d7e6fb116d45bbe15e6da84c8d23fe20582d857e4b05c809ec"
                ),
                outcome={
                    "status": "success",
                    "value": [{"page_number": 1, "chars": ["B", "e", "n", "c", "h"]}],
                },
            )
            for implementation_id in stage.semantic_implementations
        }

        self.assertEqual(
            benchmark_stages.build_timing_plan(stage, outcomes),
            stage.timed_implementations,
        )

        outcomes["pdf-oxide"]["outcome"] = {
            "status": "unsupported",
            "reason": "the pinned API cannot expose the requested stage",
        }
        outcomes["pdfsink-rs"]["outcome"]["value"][0]["chars"] = ["different"]
        self.assertEqual(
            benchmark_stages.build_timing_plan(stage, outcomes),
            ("pdfplumber-python", "pdfplumber-rs"),
        )

    def test_stage_sample_excludes_setup_and_rechecks_semantics(self) -> None:
        suite = self.load_suite()
        stage = suite.stage("serialization")
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
        sample = benchmark_stages.build_stage_sample(
            stage=stage,
            untimed_record=untimed,
            measured_outcome={"status": "success", "value": {"utf8_bytes": 37}},
            wall_time_ns=1234,
            command_argv=["adapter", "--stage", "serialization", "--timed"],
        )

        self.assertEqual(sample["measurement_scope"], "in-adapter-stage-only")
        self.assertEqual(sample["clock"], "monotonic-wall")
        self.assertEqual(sample["wall_time_ns"], 1234)
        self.assertEqual(sample["setup_operations"], list(stage.setup_operations))
        self.assertEqual(sample["timed_operation"], stage.timed_operation)
        self.assertNotIn("combined_process_wall_time_ns", sample)
        self.assertNotIn("process", sample["timed_operation"])

        with self.assertRaisesRegex(
            benchmark_stages.BenchmarkStageError,
            "timed output drifted",
        ):
            benchmark_stages.build_stage_sample(
                stage=stage,
                untimed_record=untimed,
                measured_outcome={"status": "success", "value": {"utf8_bytes": 38}},
                wall_time_ns=1234,
                command_argv=["adapter", "--stage", "serialization", "--timed"],
            )

    def test_saved_run_retains_rejections_without_stage_timings(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            output_path = Path(temporary_directory) / "stage-run.json"
            benchmark_stages.write_local_run(
                output_path,
                records=[{"case": "small-text:word-grouping", "status": "unsupported"}],
                preflight_decisions=[
                    {
                        "case_id": "small-text:word-grouping",
                        "implementation_id": "pdf-oxide",
                        "eligible_for_timing": False,
                        "reasons": ["candidate outcome is unsupported"],
                    }
                ],
                stage_timings=[],
            )

            saved = json.loads(output_path.read_text(encoding="utf-8"))
            self.assertEqual(saved["publication_status"], "local-unpublished")
            self.assertEqual(saved["stage_timings"], [])
            self.assertFalse(saved["preflight_decisions"][0]["eligible_for_timing"])
            self.assertNotIn("timings", saved)

    def test_cli_check_gates_manifest_report_adapters_and_ci(self) -> None:
        suite = self.load_suite()
        completed = subprocess.run(
            [sys.executable, str(SCRIPT_PATH), "--check"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("7 separated stages", completed.stdout)
        self.assertEqual(
            REPORT_PATH.read_text(encoding="utf-8"),
            benchmark_stages.render_markdown(suite),
        )
        workflow = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("python scripts/run_stage_benchmarks.py --check", workflow)
        self.assertIn(
            "--stage",
            (REPO_ROOT / "benchmarks" / "adapters" / "python_pdfplumber.py").read_text(
                encoding="utf-8"
            ),
        )
        self.assertIn(
            "--stage",
            (
                REPO_ROOT / "benchmarks" / "adapters" / "rust" / "src" / "main.rs"
            ).read_text(encoding="utf-8"),
        )
        self.assertIn(
            "docs/benchmarks/stages-v0.3.0.md",
            (REPO_ROOT / "README.md").read_text(encoding="utf-8"),
        )
        comparison = (REPO_ROOT / "docs" / "comparison.md").read_text(
            encoding="utf-8"
        )
        self.assertIn("benchmarks/stages-v0.3.0.md", comparison)
        self.assertIn("local and unpublished", comparison)
        roadmap = (REPO_ROOT / "ROADMAP.md").read_text(encoding="utf-8")
        self.assertIn("`SCORE-004`", roadmap)
        self.assertIn("`SCORE-005`", roadmap)
        self.assertIn("`SCORE-006`", roadmap)
        self.assertIn("`SCORE-007`", roadmap)
        self.assertIn("[`SCORE-008`]", roadmap)
        self.assertIn("[`SCORE-009`]", roadmap)
        changelog = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        self.assertIn("seven component clocks", changelog)

        for package_readme in (
            REPO_ROOT / "crates" / "pdfplumber-py" / "README.md",
            REPO_ROOT / "crates" / "pdfplumber-wasm" / "README.md",
        ):
            with self.subTest(package_readme=package_readme):
                self.assertIn(
                    "../../docs/benchmarks/stages-v0.3.0.md",
                    package_readme.read_text(encoding="utf-8"),
                )
        self.assertIn(
            "../../../docs/benchmarks/stages-v0.3.0.md",
            (
                REPO_ROOT / "crates" / "pdfplumber" / "benches" / "README.md"
            ).read_text(encoding="utf-8"),
        )


if __name__ == "__main__":
    unittest.main()
