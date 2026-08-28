"""Post-publication benchmark retention contracts (SCORE-009)."""

from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

from compat.harness import benchmark_provenance, benchmark_results, benchmark_retention
from compat.tests.test_benchmark_results import local_run

REPO_ROOT = Path(__file__).resolve().parents[2]
RETENTION_PATH = REPO_ROOT / "benchmarks" / "result-retention-v0.3.0.toml"
PUBLICATION_PATH = REPO_ROOT / "benchmarks" / "results-v0.3.0.toml"
PROVENANCE_PATH = REPO_ROOT / "benchmarks" / "provenance-v0.3.0.toml"
SCENARIOS_PATH = REPO_ROOT / "benchmarks" / "scenarios-v0.3.0.toml"
SUITE_PATH = REPO_ROOT / "benchmarks" / "competitors-v0.3.0.toml"
CORPUS_PATH = REPO_ROOT / "benchmarks" / "corpus-v0.3.0.toml"
POLICY_PATH = REPO_ROOT / "benchmarks" / "equivalence-v0.3.0.toml"
REGISTRY_PATH = REPO_ROOT / "compat" / "fixture-provenance.toml"
INDEX_PATH = REPO_ROOT / "docs" / "benchmarks" / "results-v0.3.0.md"
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "benchmark-result-audit.yml"
AUDIT_SCRIPT_PATH = REPO_ROOT / "scripts" / "audit_benchmark_results.py"
WITHDRAW_SCRIPT_PATH = REPO_ROOT / "scripts" / "withdraw_benchmark_results.py"


def load_plan() -> benchmark_retention.RetentionPlan:
    return benchmark_retention.audit_repository(
        REPO_ROOT,
        RETENTION_PATH,
        PUBLICATION_PATH,
        PROVENANCE_PATH,
        SCENARIOS_PATH,
        SUITE_PATH,
        CORPUS_PATH,
        POLICY_PATH,
        REGISTRY_PATH,
    )


def reproduction(plan: benchmark_retention.RetentionPlan) -> dict[str, object]:
    run = copy.deepcopy(local_run())
    run["run_metadata"]["source"]["revision"] = plan.source_revision
    return run


def build_test_assets(
    plan: benchmark_retention.RetentionPlan,
    output_directory: Path,
) -> tuple[benchmark_retention.RetentionPlan, dict[str, object]]:
    run = reproduction(plan)
    assets = benchmark_results.write_release_assets(
        plan.publication_plan,
        run,
        output_directory,
        release_tag=plan.release_tag,
        source_revision=plan.source_revision,
    )
    test_plan = replace(
        plan,
        raw_sha256=assets.raw_sha256,
        report_sha256=assets.report_sha256,
        checksums_sha256=hashlib.sha256(assets.checksums_bytes).hexdigest(),
    )
    return test_plan, run


class BenchmarkResultRetentionTests(unittest.TestCase):
    def test_retained_manifest_binds_current_release_and_public_index(self) -> None:
        plan = load_plan()

        self.assertEqual(plan.status, "retained")
        self.assertEqual(plan.release_tag, "benchmark-results-v0.3.0")
        self.assertEqual(
            plan.source_revision,
            "5a3807d0a99a7ee2e12632a0a07887be200c96ad",
        )
        self.assertEqual(
            plan.raw_sha256,
            "07e2f5face4b35d88a68243214c76407d7c4ff716608785968130056bc04a0b0",
        )
        self.assertEqual(
            INDEX_PATH.read_text(encoding="utf-8"),
            benchmark_retention.render_index(plan),
        )
        index = INDEX_PATH.read_text(encoding="utf-8")
        self.assertIn("Status: **retained**", index)
        self.assertIn("confirmed reproduction", index)
        self.assertIn(plan.audit_evidence_url, index)
        self.assertIn(plan.publication_plan.raw_url, index)

    def test_reproduction_retains_result_despite_new_host_timings(self) -> None:
        plan = load_plan()
        with tempfile.TemporaryDirectory() as temporary_directory:
            test_plan, reproduced = build_test_assets(
                plan,
                Path(temporary_directory),
            )
            reproduced["run_metadata"]["host"]["cpu_model"] = "Independent host"
            for timing in reproduced["scenario_timings"]:
                timing["wall_time_ns"] *= 2
            reproduced["statistical_summaries"] = (
                benchmark_provenance.summarize_samples(
                    reproduced["scenario_timings"],
                    repetitions=5,
                )
            )

            decision = benchmark_retention.audit_release_assets(
                test_plan,
                Path(temporary_directory),
                reproduced,
            )

        self.assertEqual(decision.status, "retain")
        self.assertEqual(decision.reasons, ())
        self.assertEqual(decision.semantic_record_count, 3)
        self.assertEqual(decision.timed_group_count, 2)

    def test_failed_output_equivalence_withdraws_the_whole_bundle(self) -> None:
        plan = load_plan()
        with tempfile.TemporaryDirectory() as temporary_directory:
            test_plan, reproduced = build_test_assets(
                plan,
                Path(temporary_directory),
            )
            candidate_decision = reproduced["preflight_decisions"][0]
            candidate_decision["eligible_for_timing"] = False
            candidate_decision["reasons"] = ["candidate output differs from reference"]
            candidate_decision["candidate_output_sha256"] = "9" * 64
            reproduced["scenario_timings"] = [
                timing
                for timing in reproduced["scenario_timings"]
                if timing["implementation"]["id"] != "pdfplumber-rs"
            ]
            reproduced["statistical_summaries"] = (
                benchmark_provenance.summarize_samples(
                    reproduced["scenario_timings"],
                    repetitions=5,
                )
            )

            decision = benchmark_retention.audit_release_assets(
                test_plan,
                Path(temporary_directory),
                reproduced,
            )

        self.assertEqual(decision.status, "withdraw")
        self.assertTrue(
            any("output-equivalence decision changed" in reason for reason in decision.reasons),
            decision.reasons,
        )

    def test_corrupt_or_semantically_changed_assets_are_withdrawn(self) -> None:
        plan = load_plan()
        with tempfile.TemporaryDirectory() as temporary_directory:
            output_directory = Path(temporary_directory)
            test_plan, reproduced = build_test_assets(plan, output_directory)
            raw_path = output_directory / plan.publication_plan.raw_asset
            raw_path.write_bytes(raw_path.read_bytes() + b"\n")

            corrupt = benchmark_retention.audit_release_assets(
                test_plan,
                output_directory,
                reproduced,
            )

        self.assertEqual(corrupt.status, "withdraw")
        self.assertTrue(any("raw asset SHA-256" in reason for reason in corrupt.reasons))

        with tempfile.TemporaryDirectory() as temporary_directory:
            output_directory = Path(temporary_directory)
            test_plan, reproduced = build_test_assets(plan, output_directory)
            reproduced["records"][0]["outcome"]["value"] = {"page_count": 99}

            semantic_drift = benchmark_retention.audit_release_assets(
                test_plan,
                output_directory,
                reproduced,
            )

        self.assertEqual(semantic_drift.status, "withdraw")
        self.assertTrue(
            any("semantic records changed" in reason for reason in semantic_drift.reasons)
        )

    def test_withdrawal_preserves_tag_and_audit_tombstone(self) -> None:
        plan = load_plan()
        decision = benchmark_retention.RetentionDecision(
            schema_version=1,
            status="withdraw",
            result_id=plan.id,
            release_tag=plan.release_tag,
            source_revision=plan.source_revision,
            reasons=("output-equivalence decision changed for one timed group",),
            semantic_record_count=94,
            timed_group_count=85,
        )

        tombstone = benchmark_retention.render_withdrawal_tombstone(
            plan,
            decision,
            audit_evidence_url="https://github.com/developer0hye/pdfplumber-rs/actions/runs/1",
        )
        asset_names = benchmark_retention.withdrawal_asset_names(plan)

        self.assertEqual(
            asset_names,
            (
                plan.publication_plan.raw_asset,
                plan.publication_plan.report_asset,
                plan.publication_plan.checksums_asset,
            ),
        )
        self.assertIn("withdrawn", tombstone.lower())
        self.assertIn(plan.source_revision, tombstone)
        self.assertIn("actions/runs/1", tombstone)
        self.assertIn("immutable source tag is retained", tombstone.lower())
        self.assertNotIn("delete tag", tombstone.lower())

    def test_cli_and_read_only_audit_workflow_enforce_the_policy(self) -> None:
        completed = subprocess.run(
            [sys.executable, str(AUDIT_SCRIPT_PATH), "--check"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("Benchmark result retention OK", completed.stdout)

        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        for required in (
            "schedule:",
            "workflow_dispatch:",
            "permissions:\n  contents: read",
            "ref: ${{ env.RELEASE_TAG }}",
            "gh release download",
            "run_benchmark_provenance.py --build",
            "run_benchmark_provenance.py --run",
            "audit_benchmark_results.py --audit",
            "retention-decision.json",
        ):
            self.assertIn(required, workflow)
        for prohibited in (
            "contents: write",
            "release delete",
            "release delete-asset",
            "release edit",
        ):
            self.assertNotIn(prohibited, workflow)

        withdrawal_script = WITHDRAW_SCRIPT_PATH.read_text(encoding="utf-8")
        self.assertIn('EXPECTED_GITHUB_LOGIN = "developer0hye"', withdrawal_script)
        self.assertIn("release", withdrawal_script)
        self.assertIn("delete-asset", withdrawal_script)
        self.assertIn("release", withdrawal_script)
        self.assertIn("edit", withdrawal_script)
        self.assertNotIn("delete-tag", withdrawal_script)

        ci = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("python scripts/audit_benchmark_results.py --check", ci)

        serialized = json.loads(
            benchmark_retention.serialize_decision(
                benchmark_retention.RetentionDecision(
                    schema_version=1,
                    status="retain",
                    result_id="result",
                    release_tag="benchmark-results-v0.3.0",
                    source_revision="a" * 40,
                    reasons=(),
                    semantic_record_count=94,
                    timed_group_count=85,
                )
            )
        )
        self.assertEqual(serialized["status"], "retain")
        self.assertEqual(serialized["reasons"], [])


if __name__ == "__main__":
    unittest.main()
