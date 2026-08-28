"""Release-candidate scorecard history contracts (SCORE-014)."""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from unittest import mock

from compat.harness import release_candidate_scorecards, workflow_scorecard
from compat.tests import test_compatibility_scorecard as compatibility_test_support
from compat.tests.test_benchmark_results import local_run
from scripts import build_release_candidate_scorecards

REPO_ROOT = Path(__file__).resolve().parents[2]
POLICY_PATH = REPO_ROOT / "scorecards" / "release-candidates-v0.3.0.toml"
HISTORY_PATH = REPO_ROOT / "docs" / "scorecards" / "release-candidate-history-v0.3.json"
REPORT_PATH = REPO_ROOT / "docs" / "scorecards" / "release-candidate-history-v0.3.md"
SCRIPT_PATH = REPO_ROOT / "scripts" / "build_release_candidate_scorecards.py"
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "release-candidate-scorecards.yml"

SOURCE_REVISION = "a" * 40


def policy() -> release_candidate_scorecards.HistoryPolicy:
    return release_candidate_scorecards.HistoryPolicy(
        schema_version=1,
        identifier="pdfplumber-rs-release-candidate-scorecards-v0.3",
        release_line="0.3",
        runner="macos-14",
        artifact_prefix="pdfplumber-rs-release-candidate-scorecards",
        artifact_retention_days=90,
    )


def compatibility_result() -> dict[str, object]:
    statuses = {
        "exact": 4,
        "approved_delta": 0,
        "unsupported": 1,
        "reference_failure": 0,
        "candidate_failure": 2,
        "not_tested": 0,
    }
    return {
        "schema_version": 1,
        "subject": {
            "project": "pdfplumber-rs",
            "version": "0.3.0",
            "revision": SOURCE_REVISION,
        },
        "status_vocabulary": {name: f"Meaning of {name}" for name in statuses},
        "runs": [{"id": "macos-source", "status": "observed"}],
        "observations": [
            {"id": f"observation-{index}", "status": "exact"}
            for index in range(sum(statuses.values()))
        ],
        "summary": {
            "status_counts": statuses,
            "by_api": [{"id": "extract_text", "status_counts": statuses}],
            "by_option": [],
            "by_fixture_class": [{"id": "generated", "status_counts": statuses}],
            "by_page": [],
            "by_platform": [{"id": "macos", "status_counts": statuses}],
            "by_artifact_type": [{"id": "source", "status_counts": statuses}],
        },
    }


def empty_history() -> dict[str, object]:
    return release_candidate_scorecards.empty_history(policy())


class ReleaseCandidateHistoryTests(unittest.TestCase):
    def build_entry(
        self,
        *,
        candidate_id: str = "candidate-a",
        source_revision: str = SOURCE_REVISION,
        run_url: str = "https://github.com/developer0hye/pdfplumber-rs/actions/runs/1",
    ) -> dict[str, object]:
        return release_candidate_scorecards.build_entry(
            policy(),
            candidate_id=candidate_id,
            source_revision=source_revision,
            run_url=run_url,
            benchmark_run=local_run(),
            compatibility=compatibility_result(),
            workflow_report="# Workflow scorecard\n",
        )

    def test_entry_retains_stable_benchmark_and_compatibility_dimensions(self) -> None:
        entry = self.build_entry()

        self.assertEqual(entry["source_revision"], SOURCE_REVISION)
        self.assertEqual(entry["benchmark"]["semantic_record_count"], 3)
        self.assertEqual(entry["benchmark"]["timed_group_count"], 2)
        self.assertEqual(entry["benchmark"]["raw_sample_count"], 10)
        self.assertEqual(
            entry["benchmark"]["status_counts"],
            {"success": 3},
        )
        self.assertEqual(
            entry["benchmark"]["decision_counts"],
            {"exact": 1, "rejected": 1},
        )
        self.assertEqual(
            entry["benchmark"]["statistical_summaries"],
            local_run()["statistical_summaries"],
        )
        self.assertEqual(
            entry["compatibility"]["summary"],
            compatibility_result()["summary"],
        )
        self.assertEqual(
            set(entry["compatibility"]["status_counts"]),
            {
                "exact",
                "approved_delta",
                "unsupported",
                "reference_failure",
                "candidate_failure",
                "not_tested",
            },
        )
        self.assertNotIn("percentage", json.dumps(entry).lower())

    def test_revision_or_semantic_identity_drift_is_rejected(self) -> None:
        with self.assertRaisesRegex(
            release_candidate_scorecards.ScorecardHistoryError,
            "benchmark source revision",
        ):
            self.build_entry(source_revision="b" * 40)

        scorecard = compatibility_result()
        scorecard["subject"]["revision"] = "b" * 40
        with self.assertRaisesRegex(
            release_candidate_scorecards.ScorecardHistoryError,
            "compatibility subject revision",
        ):
            release_candidate_scorecards.build_entry(
                policy(),
                candidate_id="candidate-a",
                source_revision=SOURCE_REVISION,
                run_url="https://github.com/developer0hye/pdfplumber-rs/actions/runs/1",
                benchmark_run=local_run(),
                compatibility=scorecard,
                workflow_report="# Workflow scorecard\n",
            )

        malformed = local_run()
        malformed["statistical_summaries"][0]["median_wall_time_ns"] += 1
        with self.assertRaisesRegex(
            release_candidate_scorecards.ScorecardHistoryError,
            "statistical summaries",
        ):
            release_candidate_scorecards.build_entry(
                policy(),
                candidate_id="candidate-a",
                source_revision=SOURCE_REVISION,
                run_url="https://github.com/developer0hye/pdfplumber-rs/actions/runs/1",
                benchmark_run=malformed,
                compatibility=compatibility_result(),
                workflow_report="# Workflow scorecard\n",
            )

    def test_history_is_ordered_unique_and_digest_chained(self) -> None:
        first = self.build_entry(candidate_id="candidate-a")
        history = release_candidate_scorecards.append_entry(
            policy(), empty_history(), first
        )
        second = self.build_entry(candidate_id="candidate-b")
        second["recorded_at_utc"] = "2026-08-29T00:00:00Z"
        history = release_candidate_scorecards.append_entry(policy(), history, second)

        self.assertEqual(
            history["runs"][1]["previous_entry_sha256"],
            release_candidate_scorecards.entry_sha256(history["runs"][0]),
        )
        release_candidate_scorecards.validate_history(policy(), history)

        with self.assertRaisesRegex(
            release_candidate_scorecards.ScorecardHistoryError,
            "candidate ID",
        ):
            release_candidate_scorecards.append_entry(policy(), history, second)

        older = self.build_entry(candidate_id="candidate-c")
        older["recorded_at_utc"] = "2026-08-27T00:00:00Z"
        with self.assertRaisesRegex(
            release_candidate_scorecards.ScorecardHistoryError,
            "chronological",
        ):
            release_candidate_scorecards.append_entry(policy(), history, older)

    def test_assets_are_deterministic_and_checksum_bound(self) -> None:
        entry = self.build_entry()
        history = release_candidate_scorecards.append_entry(
            policy(), empty_history(), entry
        )
        with (
            tempfile.TemporaryDirectory() as first_dir,
            tempfile.TemporaryDirectory() as second_dir,
        ):
            first = release_candidate_scorecards.write_assets(
                policy(),
                Path(first_dir),
                entry=history["runs"][-1],
                history=history,
                benchmark_run=local_run(),
                compatibility=compatibility_result(),
                workflow_report="# Workflow scorecard\n",
            )
            second = release_candidate_scorecards.write_assets(
                policy(),
                Path(second_dir),
                entry=history["runs"][-1],
                history=history,
                benchmark_run=local_run(),
                compatibility=compatibility_result(),
                workflow_report="# Workflow scorecard\n",
            )

            self.assertEqual(first.names, second.names)
            for name in first.names:
                self.assertEqual(
                    (Path(first_dir) / name).read_bytes(),
                    (Path(second_dir) / name).read_bytes(),
                )
            checksums = first.checksums_path.read_text(encoding="utf-8")
            for path in first.data_paths:
                digest = hashlib.sha256(path.read_bytes()).hexdigest()
                self.assertIn(f"{digest}  {path.name}\n", checksums)

    def test_builder_projects_raw_parity_into_machine_and_workflow_assets(
        self,
    ) -> None:
        sample_contract = compatibility_test_support.CompatibilityScorecardContractTests
        raw_run = sample_contract.observed_run()
        assert raw_run.report is not None
        repository_policy = replace(
            policy(),
            release_version="0.3.0",
            corpus=sample_contract.corpus(),
            corpus_sha256="b" * 64,
            workflow_source_path=(
                REPO_ROOT / "compat" / "workflow-scorecard-v0.3.0.toml"
            ),
        )
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "source.tar"
            wheel = Path(temporary) / "candidate.whl"
            source.write_bytes(b"source")
            wheel.write_bytes(b"wheel")
            definitions = (
                workflow_scorecard.WorkflowDefinition(
                    identifier="open", title="Open", projection="document_open"
                ),
                workflow_scorecard.WorkflowDefinition(
                    identifier="text",
                    title="Text",
                    api_ids=(
                        "chars",
                        "layout_text",
                        "page_text",
                        "simple_text",
                        "text_lines",
                    ),
                ),
                workflow_scorecard.WorkflowDefinition(
                    identifier="words",
                    title="Words",
                    api_ids=("extract_words", "words"),
                ),
                workflow_scorecard.WorkflowDefinition(
                    identifier="crop", title="Crop", not_tested_reason="not observed"
                ),
                workflow_scorecard.WorkflowDefinition(
                    identifier="search", title="Search", api_ids=("search",)
                ),
                workflow_scorecard.WorkflowDefinition(
                    identifier="tables", title="Tables", api_ids=("tables",)
                ),
                workflow_scorecard.WorkflowDefinition(
                    identifier="serialization",
                    title="Serialization",
                    not_tested_reason="not observed",
                ),
                workflow_scorecard.WorkflowDefinition(
                    identifier="annotations",
                    title="Annotations",
                    api_ids=("annotations", "hyperlinks"),
                ),
                workflow_scorecard.WorkflowDefinition(
                    identifier="structure",
                    title="Structure",
                    api_ids=("structure_tree",),
                ),
                workflow_scorecard.WorkflowDefinition(
                    identifier="rendering",
                    title="Rendering",
                    not_tested_reason="not observed",
                ),
                workflow_scorecard.WorkflowDefinition(
                    identifier="cli",
                    title="Command-Line Interface",
                    not_tested_reason="not observed",
                ),
            )
            with (
                mock.patch.object(
                    build_release_candidate_scorecards,
                    "command_output",
                    side_effect=(
                        "rustc 1.98.0 (88d9e12ae 2026-08-18)",
                        "cargo 1.98.0 (797e8a9bc 2026-08-05)",
                        "maturin 1.14.1",
                    ),
                ),
                mock.patch.object(
                    build_release_candidate_scorecards.generate_workflow_scorecard,
                    "workflow_definitions",
                    return_value=definitions,
                ),
            ):
                machine, workflow = (
                    build_release_candidate_scorecards.build_compatibility(
                        repository_policy,
                        candidate_id="candidate-a",
                        source_revision=SOURCE_REVISION,
                        source_artifact=source,
                        wheel=wheel,
                        parity_report=dict(raw_run.report),
                    )
                )

        self.assertEqual(machine["subject"]["revision"], SOURCE_REVISION)
        self.assertEqual(
            {(run["artifact_type"], tuple(run["scopes"])) for run in machine["runs"]},
            {("source", ("api",)), ("wheel", ("option",))},
        )
        self.assertTrue(machine["observations"])
        self.assertIn("# Compatibility workflows", workflow)
        self.assertIn("No success percentage is computed", workflow)
        self.assertNotIn("%", workflow)

    def test_human_history_reports_counts_without_a_success_percentage(self) -> None:
        history = release_candidate_scorecards.append_entry(
            policy(), empty_history(), self.build_entry()
        )

        report = release_candidate_scorecards.render_report(policy(), history)

        self.assertIn("candidate-a", report)
        self.assertIn("Exact", report)
        self.assertIn("Candidate failure", report)
        self.assertIn("Timed groups", report)
        self.assertNotIn("%", report)
        self.assertNotIn("percentage", report.lower())

    def test_repository_policy_gates_release_and_retains_public_assets(self) -> None:
        for path in (
            POLICY_PATH,
            HISTORY_PATH,
            REPORT_PATH,
            SCRIPT_PATH,
            WORKFLOW_PATH,
        ):
            with self.subTest(path=path):
                self.assertTrue(
                    path.is_file(), f"missing {path.relative_to(REPO_ROOT)}"
                )

        completed = subprocess.run(
            [sys.executable, str(SCRIPT_PATH), "--check"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)

        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        for required in (
            "workflow_call:",
            "workflow_dispatch:",
            "runs-on: macos-14",
            "python scripts/run_benchmark_provenance.py --build",
            "python scripts/run_benchmark_provenance.py --run",
            "scripts/setup_golden_venv.sh",
            "scripts/parity_report.py",
            "continue-on-error: true",
            "scripts/build_release_candidate_scorecards.py --build-assets",
            "retention-days: 90",
        ):
            self.assertIn(required, workflow)

        release = (REPO_ROOT / ".github" / "workflows" / "release.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn(
            "uses: ./.github/workflows/release-candidate-scorecards.yml",
            release,
        )
        self.assertIn("needs: [ci, metadata, scorecards]", release)
        self.assertIn("pattern: release-candidate-scorecards-*", release)
        self.assertIn("release-scorecards/*", release)

        ci = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn(
            "python scripts/build_release_candidate_scorecards.py --check",
            ci,
        )

    def test_observed_gap_producers_do_not_block_scorecard_asset_building(self) -> None:
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")

        for step_name in (
            "Generate candidate option results",
            "Re-run the complete compatibility scorecard input",
        ):
            with self.subTest(step_name=step_name):
                marker = f"      - name: {step_name}\n"
                self.assertIn(marker, workflow)
                step = workflow.split(marker, maxsplit=1)[1].split(
                    "\n      - name:", maxsplit=1
                )[0]
                self.assertIn("continue-on-error: true", step)


if __name__ == "__main__":
    unittest.main()
