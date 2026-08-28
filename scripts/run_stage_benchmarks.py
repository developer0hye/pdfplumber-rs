#!/usr/bin/env python3
"""Validate, build, or execute the separated SCORE-004 stage suite."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections.abc import Mapping
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from compat.harness import benchmark_stages

STAGES_PATH = REPO_ROOT / "benchmarks" / "stages-v0.3.0.toml"
SUITE_PATH = REPO_ROOT / "benchmarks" / "competitors-v0.3.0.toml"
CORPUS_PATH = REPO_ROOT / "benchmarks" / "corpus-v0.3.0.toml"
POLICY_PATH = REPO_ROOT / "benchmarks" / "equivalence-v0.3.0.toml"
REGISTRY_PATH = REPO_ROOT / "compat" / "fixture-provenance.toml"
REPORT_PATH = REPO_ROOT / "docs" / "benchmarks" / "stages-v0.3.0.md"
DEFAULT_REFERENCE_PYTHON = REPO_ROOT / ".venv-reference" / "bin" / "python"
DEFAULT_CANDIDATE_PYTHON = REPO_ROOT / ".venv-candidate" / "bin" / "python"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument("--check", action="store_true")
    modes.add_argument("--write-report", action="store_true")
    modes.add_argument("--build", action="store_true")
    modes.add_argument("--run", action="store_true")
    parser.add_argument("--output", type=Path)
    parser.add_argument(
        "--reference-python",
        type=Path,
        default=DEFAULT_REFERENCE_PYTHON,
    )
    parser.add_argument(
        "--candidate-python",
        type=Path,
        default=DEFAULT_CANDIDATE_PYTHON,
    )
    return parser.parse_args()


def load_suite() -> benchmark_stages.StageSuite:
    return benchmark_stages.audit_repository(
        REPO_ROOT,
        STAGES_PATH,
        SUITE_PATH,
        CORPUS_PATH,
        POLICY_PATH,
        REGISTRY_PATH,
    )


def check_suite(suite: benchmark_stages.StageSuite) -> None:
    if not REPORT_PATH.is_file():
        raise benchmark_stages.BenchmarkStageError(
            f"stage report is missing: {REPORT_PATH}"
        )
    if REPORT_PATH.read_text(encoding="utf-8") != benchmark_stages.render_markdown(
        suite
    ):
        raise benchmark_stages.BenchmarkStageError(
            f"stage report is stale: {REPORT_PATH}"
        )
    adapter_sources = (
        REPO_ROOT / suite.competitor_suite.python_adapter,
        REPO_ROOT / suite.candidate_python_adapter,
        (REPO_ROOT / suite.competitor_suite.rust_manifest).parent
        / "src"
        / "main.rs",
    )
    for adapter_source in adapter_sources:
        source = adapter_source.read_text(encoding="utf-8")
        if "--stage" not in source or "--timed" not in source:
            raise benchmark_stages.BenchmarkStageError(
                f"adapter lacks the stage timing protocol: {adapter_source}"
            )


def build_adapters() -> None:
    subprocess.run(
        [sys.executable, "scripts/run_competitor_benchmarks.py", "--build"],
        cwd=REPO_ROOT,
        check=True,
    )


def candidate_revision() -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout.strip()


def implementation_revision(
    suite: benchmark_stages.StageSuite,
    implementation_id: str,
    resolved_candidate_revision: str,
) -> str:
    if implementation_id in {"pdfplumber-rs", suite.candidate_python_id}:
        return resolved_candidate_revision
    return suite.competitor_suite.implementation(implementation_id).revision


def adapter_command(
    suite: benchmark_stages.StageSuite,
    implementation_id: str,
    stage_id: str,
    fixture_path: str,
    fixture_password: str | None,
    reference_python: Path,
    candidate_python: Path,
    *,
    timed: bool,
) -> list[str]:
    if implementation_id == suite.candidate_python_id:
        command = [str(candidate_python), suite.candidate_python_adapter]
    else:
        implementation = suite.competitor_suite.implementation(implementation_id)
        command = [
            str(reference_python) if argument == "{reference_python}" else argument
            for argument in implementation.command
        ]
    command.extend(("--stage", stage_id, "--fixture", fixture_path))
    if fixture_password is not None:
        command.extend(("--password", fixture_password))
    if timed:
        command.append("--timed")
    return command


def run_adapter(command: list[str]) -> dict[str, object]:
    completed = subprocess.run(
        command,
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        message = completed.stderr.strip().splitlines()
        return {
            "status": "error",
            "error": {
                "kind": "adapter-process",
                "message": message[-1] if message else f"exit {completed.returncode}",
            },
        }
    try:
        decoded = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise benchmark_stages.BenchmarkStageError(
            f"adapter emitted invalid JSON: {command[0]}"
        ) from error
    if not isinstance(decoded, dict):
        raise benchmark_stages.BenchmarkStageError(
            "stage adapter outcome must be one object"
        )
    return decoded


def semantic_outcome(decoded: Mapping[str, object]) -> dict[str, object]:
    outcome = dict(decoded)
    timing = outcome.pop("timing", None)
    if timing is not None:
        raise benchmark_stages.BenchmarkStageError(
            "untimed semantic invocation emitted a timing"
        )
    return outcome


def measured_outcome(
    decoded: Mapping[str, object],
    stage_id: str,
) -> tuple[dict[str, object], int]:
    outcome = dict(decoded)
    timing = outcome.pop("timing", None)
    if not isinstance(timing, dict) or set(timing) != {
        "stage_id",
        "clock",
        "wall_time_ns",
    }:
        raise benchmark_stages.BenchmarkStageError(
            f"timed adapter lacks an exact timing envelope for {stage_id}"
        )
    if timing.get("stage_id") != stage_id or timing.get("clock") != "monotonic-wall":
        raise benchmark_stages.BenchmarkStageError(
            f"timed adapter names the wrong stage or clock for {stage_id}"
        )
    wall_time_ns = timing.get("wall_time_ns")
    if (
        isinstance(wall_time_ns, bool)
        or not isinstance(wall_time_ns, int)
        or wall_time_ns <= 0
    ):
        raise benchmark_stages.BenchmarkStageError(
            f"timed adapter emitted invalid wall_time_ns for {stage_id}"
        )
    return outcome, wall_time_ns


def execute_local_run(
    suite: benchmark_stages.StageSuite,
    reference_python: Path,
    candidate_python: Path,
) -> tuple[
    list[dict[str, object]],
    list[dict[str, object]],
    list[dict[str, object]],
]:
    if not reference_python.is_file():
        raise benchmark_stages.BenchmarkStageError(
            f"reference Python is missing: {reference_python}"
        )
    if not candidate_python.is_file():
        raise benchmark_stages.BenchmarkStageError(
            f"candidate Python is missing: {candidate_python}"
        )
    binary_path = REPO_ROOT / suite.competitor_suite.rust_binary
    if not binary_path.is_file():
        raise benchmark_stages.BenchmarkStageError(
            "Rust benchmark adapter is missing; run --build first"
        )

    resolved_candidate_revision = candidate_revision()
    fixtures = {
        fixture.id: fixture for fixture in suite.competitor_suite.corpus.fixtures
    }
    records: list[dict[str, object]] = []
    records_by_key: dict[
        tuple[str, str], dict[str, dict[str, object]]
    ] = {}

    # Every semantic result completes before any stage clock starts.
    for stage in suite.stages:
        for fixture_id in stage.fixture_ids:
            fixture = fixtures[fixture_id]
            for implementation_id in stage.semantic_implementations:
                command = adapter_command(
                    suite,
                    implementation_id,
                    stage.id,
                    fixture.path,
                    fixture.password,
                    reference_python,
                    candidate_python,
                    timed=False,
                )
                outcome = semantic_outcome(run_adapter(command))
                record = benchmark_stages.synthetic_stage_record(
                    stage=stage,
                    implementation_id=implementation_id,
                    revision=implementation_revision(
                        suite,
                        implementation_id,
                        resolved_candidate_revision,
                    ),
                    fixture_id=fixture.id,
                    fixture_sha256=fixture.sha256,
                    outcome=outcome,
                )
                records.append(record)
                records_by_key.setdefault((stage.id, fixture.id), {})[
                    implementation_id
                ] = record

    decisions: list[dict[str, object]] = []
    eligible_by_key: dict[tuple[str, str], tuple[str, ...]] = {}
    for stage in suite.stages:
        for fixture_id in stage.fixture_ids:
            key = (stage.id, fixture_id)
            case_records = records_by_key[key]
            reference = case_records[stage.semantic_reference]
            decisions_by_implementation: dict[str, dict[str, object]] = {}
            for implementation_id in stage.semantic_implementations:
                if implementation_id == stage.semantic_reference:
                    continue
                decision = benchmark_stages.preflight(
                    stage,
                    reference,
                    case_records[implementation_id],
                )
                decisions.append(decision)
                decisions_by_implementation[implementation_id] = decision

            eligible_non_reference = [
                implementation_id
                for implementation_id in stage.timed_implementations
                if implementation_id != stage.semantic_reference
                and decisions_by_implementation[implementation_id][
                    "eligible_for_timing"
                ]
            ]
            eligible: list[str] = []
            if (
                stage.semantic_reference in stage.timed_implementations
                and eligible_non_reference
            ):
                eligible.append(stage.semantic_reference)
            eligible.extend(eligible_non_reference)
            eligible_by_key[key] = tuple(eligible)

    stage_timings: list[dict[str, object]] = []
    for stage in suite.stages:
        for fixture_id in stage.fixture_ids:
            fixture = fixtures[fixture_id]
            key = (stage.id, fixture_id)
            for implementation_id in eligible_by_key[key]:
                command = adapter_command(
                    suite,
                    implementation_id,
                    stage.id,
                    fixture.path,
                    fixture.password,
                    reference_python,
                    candidate_python,
                    timed=True,
                )
                outcome, wall_time_ns = measured_outcome(
                    run_adapter(command),
                    stage.id,
                )
                stage_timings.append(
                    benchmark_stages.build_stage_sample(
                        stage=stage,
                        untimed_record=records_by_key[key][implementation_id],
                        measured_outcome=outcome,
                        wall_time_ns=wall_time_ns,
                        command_argv=command,
                    )
                )
    return records, decisions, stage_timings


def write_report(suite: benchmark_stages.StageSuite) -> None:
    REPORT_PATH.parent.mkdir(parents=True, exist_ok=True)
    REPORT_PATH.write_text(
        benchmark_stages.render_markdown(suite),
        encoding="utf-8",
    )


def main() -> int:
    args = parse_args()
    if args.run != (args.output is not None):
        print("--output is required exactly with --run", file=sys.stderr)
        return 2
    try:
        suite = load_suite()
        if args.write_report:
            write_report(suite)
            print(f"Wrote {REPORT_PATH.relative_to(REPO_ROOT)}")
            return 0
        if args.check:
            check_suite(suite)
            print(f"Stage suite OK: {len(suite.stages)} separated stages")
            return 0
        if args.build:
            build_adapters()
            print(f"Built {suite.competitor_suite.rust_binary}")
            return 0

        assert args.output is not None
        records, decisions, stage_timings = execute_local_run(
            suite,
            args.reference_python,
            args.candidate_python,
        )
        benchmark_stages.write_local_run(
            args.output,
            records=records,
            preflight_decisions=decisions,
            stage_timings=stage_timings,
        )
        print(
            f"Wrote local unpublished stage run: {len(records)} records, "
            f"{len(stage_timings)} separated samples"
        )
        return 0
    except (
        benchmark_stages.BenchmarkStageError,
        OSError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"Stage benchmark error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
