#!/usr/bin/env python3
"""Validate, build, or execute the local pinned competitor suite."""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
import time
from collections.abc import Mapping
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from compat.harness import benchmark_competitors, benchmark_equivalence

SUITE_PATH = REPO_ROOT / "benchmarks" / "competitors-v0.3.0.toml"
CORPUS_PATH = REPO_ROOT / "benchmarks" / "corpus-v0.3.0.toml"
POLICY_PATH = REPO_ROOT / "benchmarks" / "equivalence-v0.3.0.toml"
REGISTRY_PATH = REPO_ROOT / "compat" / "fixture-provenance.toml"
REPORT_PATH = REPO_ROOT / "docs" / "benchmarks" / "competitors-v0.3.0.md"
DEFAULT_REFERENCE_PYTHON = REPO_ROOT / ".venv-reference" / "bin" / "python"


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
    return parser.parse_args()


def load_suite() -> benchmark_competitors.CompetitorSuite:
    return benchmark_competitors.audit_repository(
        REPO_ROOT,
        SUITE_PATH,
        CORPUS_PATH,
        POLICY_PATH,
        REGISTRY_PATH,
    )


def check_suite(suite: benchmark_competitors.CompetitorSuite) -> None:
    expected = benchmark_competitors.render_markdown(suite)
    if not REPORT_PATH.is_file():
        raise benchmark_competitors.CompetitorBenchmarkError(
            f"competitor report is missing: {REPORT_PATH}"
        )
    if REPORT_PATH.read_text(encoding="utf-8") != expected:
        raise benchmark_competitors.CompetitorBenchmarkError(
            f"competitor report is stale: {REPORT_PATH}"
        )
    lock_path = REPO_ROOT / suite.rust_manifest
    lock_path = lock_path.parent / "Cargo.lock"
    if not lock_path.is_file():
        raise benchmark_competitors.CompetitorBenchmarkError(
            f"pinned adapter lock is missing: {lock_path}"
        )
    if "pdf_oxide" not in lock_path.read_text(encoding="utf-8"):
        raise benchmark_competitors.CompetitorBenchmarkError(
            "adapter lock does not retain the prepared pdf_oxide package"
        )


def build_adapters(suite: benchmark_competitors.CompetitorSuite) -> None:
    prepare_sources(suite)
    subprocess.run(
        [
            "cargo",
            "build",
            "--manifest-path",
            suite.rust_manifest,
            "--target-dir",
            str((REPO_ROOT / suite.rust_binary).parents[1]),
            "--release",
            "--locked",
        ],
        cwd=REPO_ROOT,
        check=True,
    )


def prepare_sources(suite: benchmark_competitors.CompetitorSuite) -> None:
    """Materialize shallow exact-revision sources for Cargo path dependencies."""

    adapter_directory = (REPO_ROOT / suite.rust_manifest).parent
    source_root = adapter_directory.parent / ".sources"
    source_root.mkdir(parents=True, exist_ok=True)
    source_specs = (
        ("pdf-oxide", source_root / "pdf_oxide"),
        ("pdfsink-rs", source_root / "pdfsink-rs"),
    )
    for implementation_id, destination in source_specs:
        implementation = suite.implementation(implementation_id)
        if destination.is_dir():
            completed = subprocess.run(
                ["git", "rev-parse", "HEAD"],
                cwd=destination,
                check=True,
                capture_output=True,
                text=True,
            )
            if completed.stdout.strip() != implementation.revision:
                raise benchmark_competitors.CompetitorBenchmarkError(
                    f"prepared {implementation_id} source has the wrong revision"
                )
            continue
        if destination.exists():
            raise benchmark_competitors.CompetitorBenchmarkError(
                f"prepared source path is not a directory: {destination}"
            )
        with tempfile.TemporaryDirectory(
            prefix=f"pdfplumber-rs-{implementation_id}-"
        ) as temporary_directory:
            checkout = Path(temporary_directory) / "source"
            checkout.mkdir()
            subprocess.run(["git", "init"], cwd=checkout, check=True)
            subprocess.run(
                ["git", "remote", "add", "origin", implementation.repository],
                cwd=checkout,
                check=True,
            )
            subprocess.run(
                [
                    "git",
                    "fetch",
                    "--depth=1",
                    "origin",
                    implementation.revision,
                ],
                cwd=checkout,
                check=True,
            )
            subprocess.run(
                ["git", "checkout", "--detach", implementation.revision],
                cwd=checkout,
                check=True,
            )
            shutil.move(str(checkout), destination)


def candidate_revision() -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout.strip()


def adapter_command(
    suite: benchmark_competitors.CompetitorSuite,
    case: benchmark_competitors.CompetitorCase,
    reference_python: Path,
) -> list[str]:
    implementation = suite.implementation(case.implementation_id)
    command = [
        str(reference_python) if argument == "{reference_python}" else argument
        for argument in implementation.command
    ]
    command.extend(
        (
            "--workload",
            case.workload_id,
            "--fixture",
            case.fixture_path,
        )
    )
    if case.fixture_password is not None:
        command.extend(("--password", case.fixture_password))
    return command


def run_adapter(
    suite: benchmark_competitors.CompetitorSuite,
    case: benchmark_competitors.CompetitorCase,
    reference_python: Path,
) -> tuple[dict[str, object], list[str]]:
    command = adapter_command(suite, case, reference_python)
    completed = subprocess.run(
        command,
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        message = completed.stderr.strip().splitlines()
        outcome: dict[str, object] = {
            "status": "error",
            "error": {
                "kind": "adapter-process",
                "message": message[-1] if message else f"exit {completed.returncode}",
            },
        }
        return outcome, command
    try:
        decoded = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise benchmark_competitors.CompetitorBenchmarkError(
            f"{case.implementation_id} emitted invalid JSON for "
            f"{case.fixture_id}:{case.workload_id}"
        ) from error
    if not isinstance(decoded, dict):
        raise benchmark_competitors.CompetitorBenchmarkError(
            f"{case.implementation_id} adapter outcome must be one object"
        )
    return decoded, command


def record_for_case(
    case: benchmark_competitors.CompetitorCase,
    outcome: Mapping[str, object],
    resolved_candidate_revision: str,
) -> dict[str, object]:
    revision = (
        resolved_candidate_revision
        if case.implementation_revision == "repository-head"
        else case.implementation_revision
    )
    return benchmark_competitors.synthetic_record(
        implementation_id=case.implementation_id,
        revision=revision,
        fixture_id=case.fixture_id,
        fixture_sha256=case.fixture_sha256,
        workload_id=case.workload_id,
        outcome=outcome,
    )


def execute_local_run(
    suite: benchmark_competitors.CompetitorSuite,
    reference_python: Path,
) -> tuple[
    list[dict[str, object]],
    list[dict[str, object]],
    list[dict[str, object]],
]:
    if not reference_python.is_file():
        raise benchmark_competitors.CompetitorBenchmarkError(
            f"reference Python is missing: {reference_python}"
        )
    binary_path = REPO_ROOT / suite.rust_binary
    if not binary_path.is_file():
        raise benchmark_competitors.CompetitorBenchmarkError(
            "Rust benchmark adapter is missing; run --build first"
        )

    resolved_candidate_revision = candidate_revision()
    cases = benchmark_competitors.expand_cases(suite)
    records: list[dict[str, object]] = []
    cases_by_key: dict[
        tuple[str, str],
        dict[str, benchmark_competitors.CompetitorCase],
    ] = {}
    records_by_key: dict[
        tuple[str, str],
        dict[str, dict[str, object]],
    ] = {}

    # Complete and retain every semantic result before starting any clock.
    for case in cases:
        outcome, _ = run_adapter(suite, case, reference_python)
        record = record_for_case(case, outcome, resolved_candidate_revision)
        records.append(record)
        key = (case.fixture_id, case.workload_id)
        cases_by_key.setdefault(key, {})[case.implementation_id] = case
        records_by_key.setdefault(key, {})[case.implementation_id] = record

    decisions: list[dict[str, object]] = []
    timing_groups: dict[tuple[str, str], tuple[tuple[str, str, str], ...]] = {}
    for key in sorted(records_by_key):
        case_records = records_by_key[key]
        reference_record = case_records[benchmark_competitors.REFERENCE_IMPLEMENTATION]
        for implementation_id in (
            benchmark_competitors.CANDIDATE_IMPLEMENTATION,
            *benchmark_competitors.COMPETITOR_IMPLEMENTATIONS,
        ):
            decision = benchmark_equivalence.preflight(
                reference_record,
                case_records[implementation_id],
                suite.policy,
                suite.corpus,
            )
            rendered = decision.to_dict()
            rendered["implementation_id"] = implementation_id
            decisions.append(rendered)
        timing_groups[key] = benchmark_competitors.build_timing_plan(case_records)

    timings: list[dict[str, object]] = []
    for key in sorted(timing_groups):
        groups = timing_groups[key]
        implementation_ids = sorted(
            {implementation_id for group in groups for implementation_id in group}
        )
        for implementation_id in implementation_ids:
            case = cases_by_key[key][implementation_id]
            started_ns = time.perf_counter_ns()
            measured_outcome, command = run_adapter(suite, case, reference_python)
            elapsed_ns = time.perf_counter_ns() - started_ns
            measured_record = record_for_case(
                case,
                measured_outcome,
                resolved_candidate_revision,
            )
            if (
                measured_record["outcome"]
                != records_by_key[key][implementation_id]["outcome"]
            ):
                raise benchmark_competitors.CompetitorBenchmarkError(
                    f"timed output drifted for {implementation_id} {key[0]}:{key[1]}"
                )
            timings.append(
                {
                    "case_id": f"{key[0]}:{key[1]}",
                    "implementation_id": implementation_id,
                    "implementation_revision": measured_record["implementation"][
                        "revision"
                    ],
                    "measurement_scope": "combined-process-single-sample",
                    "combined_process_wall_time_ns": elapsed_ns,
                    "command_argv": command,
                }
            )
    return records, decisions, timings


def write_report(suite: benchmark_competitors.CompetitorSuite) -> None:
    REPORT_PATH.parent.mkdir(parents=True, exist_ok=True)
    REPORT_PATH.write_text(
        benchmark_competitors.render_markdown(suite),
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
            case_count = len(benchmark_competitors.expand_cases(suite))
            print(
                f"Competitor suite OK: {len(suite.implementations)} pinned "
                f"implementations, {case_count} untimed cases"
            )
            return 0
        if args.build:
            build_adapters(suite)
            print(f"Built {suite.rust_binary}")
            return 0

        assert args.output is not None
        records, decisions, timings = execute_local_run(
            suite,
            args.reference_python,
        )
        benchmark_competitors.write_local_run(
            args.output,
            records=records,
            preflight_decisions=decisions,
            timings=timings,
        )
        print(
            f"Wrote local unpublished run: {len(records)} records, "
            f"{len(timings)} timing samples"
        )
        return 0
    except (
        benchmark_competitors.CompetitorBenchmarkError,
        benchmark_equivalence.BenchmarkEquivalenceError,
        OSError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"Competitor benchmark error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
