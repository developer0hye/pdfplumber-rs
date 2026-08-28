#!/usr/bin/env python3
"""Validate, build, or execute the local SCORE-006 scenario suite."""

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

from compat.harness import benchmark_provenance, benchmark_scenarios

SCENARIOS_PATH = REPO_ROOT / "benchmarks" / "scenarios-v0.3.0.toml"
SUITE_PATH = REPO_ROOT / "benchmarks" / "competitors-v0.3.0.toml"
CORPUS_PATH = REPO_ROOT / "benchmarks" / "corpus-v0.3.0.toml"
POLICY_PATH = REPO_ROOT / "benchmarks" / "equivalence-v0.3.0.toml"
REGISTRY_PATH = REPO_ROOT / "compat" / "fixture-provenance.toml"
REPORT_PATH = REPO_ROOT / "docs" / "benchmarks" / "scenarios-v0.3.0.md"
DEFAULT_REFERENCE_PYTHON = REPO_ROOT / ".venv-reference" / "bin" / "python"
DEFAULT_CANDIDATE_PYTHON = REPO_ROOT / ".venv-candidate" / "bin" / "python"
REFERENCE_IMPLEMENTATION = "pdfplumber-python"


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


def load_suite() -> benchmark_scenarios.ScenarioSuite:
    return benchmark_scenarios.audit_repository(
        REPO_ROOT,
        SCENARIOS_PATH,
        SUITE_PATH,
        CORPUS_PATH,
        POLICY_PATH,
        REGISTRY_PATH,
    )


def check_suite(suite: benchmark_scenarios.ScenarioSuite) -> None:
    if not REPORT_PATH.is_file():
        raise benchmark_scenarios.BenchmarkScenarioError(
            f"scenario report is missing: {REPORT_PATH}"
        )
    if REPORT_PATH.read_text(encoding="utf-8") != benchmark_scenarios.render_markdown(
        suite
    ):
        raise benchmark_scenarios.BenchmarkScenarioError(
            f"scenario report is stale: {REPORT_PATH}"
        )
    for adapter_path in (
        REPO_ROOT / suite.competitor_suite.python_adapter,
        REPO_ROOT / suite.candidate_python_adapter,
        (REPO_ROOT / suite.competitor_suite.rust_manifest).parent / "src" / "main.rs",
    ):
        if "--scenario" not in adapter_path.read_text(encoding="utf-8"):
            raise benchmark_scenarios.BenchmarkScenarioError(
                f"adapter lacks scenario protocol: {adapter_path}"
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
    suite: benchmark_scenarios.ScenarioSuite,
    implementation_id: str,
    resolved_candidate_revision: str,
) -> str:
    if implementation_id in {
        suite.competitor_suite.candidate_implementation,
        suite.candidate_python_id,
    }:
        return resolved_candidate_revision
    return suite.competitor_suite.implementation(implementation_id).revision


def adapter_command(
    suite: benchmark_scenarios.ScenarioSuite,
    implementation_id: str,
    scenario_id: str,
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
    command.extend(("--scenario", scenario_id, "--fixture", fixture_path))
    if fixture_password is not None:
        command.extend(("--password", fixture_password))
    if timed:
        command.append("--timed")
    return command


def recorded_command_argv(command: list[str]) -> list[str]:
    """Make repository-owned command paths relocatable from repository root."""

    return benchmark_provenance.recorded_argv(command, REPO_ROOT)


def run_adapter(command: list[str]) -> dict[str, object]:
    completed = subprocess.run(
        command,
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        messages = completed.stderr.strip().splitlines()
        return {
            "status": "error",
            "error": {
                "kind": "adapter-process",
                "message": messages[-1] if messages else f"exit {completed.returncode}",
            },
        }
    try:
        decoded = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise benchmark_scenarios.BenchmarkScenarioError(
            f"adapter emitted invalid JSON: {command[0]}"
        ) from error
    if not isinstance(decoded, dict):
        raise benchmark_scenarios.BenchmarkScenarioError(
            "scenario adapter outcome must be one object"
        )
    return decoded


def semantic_outcome(decoded: Mapping[str, object]) -> dict[str, object]:
    outcome = dict(decoded)
    if outcome.pop("timing", None) is not None:
        raise benchmark_scenarios.BenchmarkScenarioError(
            "untimed semantic invocation emitted a timing"
        )
    return outcome


def measured_outcome(
    decoded: Mapping[str, object], scenario_id: str
) -> tuple[dict[str, object], int]:
    outcome = dict(decoded)
    timing = outcome.pop("timing", None)
    if not isinstance(timing, dict) or set(timing) != {
        "scenario_id",
        "clock",
        "wall_time_ns",
    }:
        raise benchmark_scenarios.BenchmarkScenarioError(
            f"adapter lacks an exact scenario timing envelope for {scenario_id}"
        )
    if (
        timing.get("scenario_id") != scenario_id
        or timing.get("clock") != "monotonic-wall"
    ):
        raise benchmark_scenarios.BenchmarkScenarioError(
            f"adapter names the wrong scenario or clock for {scenario_id}"
        )
    wall_time_ns = timing.get("wall_time_ns")
    if (
        isinstance(wall_time_ns, bool)
        or not isinstance(wall_time_ns, int)
        or wall_time_ns <= 0
    ):
        raise benchmark_scenarios.BenchmarkScenarioError(
            f"adapter emitted invalid wall_time_ns for {scenario_id}"
        )
    return outcome, wall_time_ns


def execute_local_run(
    suite: benchmark_scenarios.ScenarioSuite,
    reference_python: Path,
    candidate_python: Path,
    *,
    repetitions: int = 1,
) -> tuple[
    list[dict[str, object]],
    list[dict[str, object]],
    list[dict[str, object]],
]:
    if isinstance(repetitions, bool) or not isinstance(repetitions, int):
        raise benchmark_scenarios.BenchmarkScenarioError(
            "repetitions must be an integer"
        )
    if repetitions <= 0:
        raise benchmark_scenarios.BenchmarkScenarioError("repetitions must be positive")
    if not reference_python.is_file():
        raise benchmark_scenarios.BenchmarkScenarioError(
            f"reference Python is missing: {reference_python}"
        )
    if not candidate_python.is_file():
        raise benchmark_scenarios.BenchmarkScenarioError(
            f"candidate Python is missing: {candidate_python}"
        )
    binary_path = REPO_ROOT / suite.competitor_suite.rust_binary
    if not binary_path.is_file():
        raise benchmark_scenarios.BenchmarkScenarioError(
            "Rust benchmark adapter is missing; run --build first"
        )

    resolved_candidate_revision = candidate_revision()
    records: list[dict[str, object]] = []
    records_by_key: dict[
        tuple[str, str], dict[str, dict[str, object]]
    ] = {}
    for scenario in suite.scenarios:
        for fixture_id in scenario.fixture_ids:
            fixture = suite.competitor_suite.corpus.fixture(fixture_id)
            key = (scenario.id, fixture.id)
            for implementation_id in scenario.semantic_implementations:
                command = adapter_command(
                    suite,
                    implementation_id,
                    scenario.id,
                    fixture.path,
                    fixture.password,
                    reference_python,
                    candidate_python,
                    timed=False,
                )
                outcome = semantic_outcome(run_adapter(command))
                record = benchmark_scenarios.synthetic_scenario_record(
                    scenario=scenario,
                    implementation_id=implementation_id,
                    revision=implementation_revision(
                        suite,
                        implementation_id,
                        resolved_candidate_revision,
                    ),
                    fixtures=((fixture.id, fixture.sha256),),
                    outcome=outcome,
                )
                records.append(record)
                records_by_key.setdefault(key, {})[implementation_id] = record

    decisions: list[dict[str, object]] = []
    eligible_by_key: dict[tuple[str, str], tuple[str, ...]] = {}
    for scenario in suite.scenarios:
        for fixture_id in scenario.fixture_ids:
            key = (scenario.id, fixture_id)
            case_records = records_by_key[key]
            reference = case_records[REFERENCE_IMPLEMENTATION]
            decisions_by_implementation: dict[str, dict[str, object]] = {}
            for implementation_id in scenario.semantic_implementations:
                if implementation_id == REFERENCE_IMPLEMENTATION:
                    continue
                decision = benchmark_scenarios.preflight(
                    scenario,
                    reference,
                    case_records[implementation_id],
                )
                decisions.append(decision)
                decisions_by_implementation[implementation_id] = decision

            eligible_non_reference = [
                implementation_id
                for implementation_id in scenario.timed_implementations
                if implementation_id != REFERENCE_IMPLEMENTATION
                and decisions_by_implementation[implementation_id][
                    "eligible_for_timing"
                ]
            ]
            eligible: list[str] = []
            if (
                REFERENCE_IMPLEMENTATION in scenario.timed_implementations
                and eligible_non_reference
            ):
                eligible.append(REFERENCE_IMPLEMENTATION)
            eligible.extend(eligible_non_reference)
            eligible_by_key[key] = tuple(eligible)

    scenario_timings: list[dict[str, object]] = []
    for repetition in range(1, repetitions + 1):
        for scenario in suite.scenarios:
            for fixture_id in scenario.fixture_ids:
                fixture = suite.competitor_suite.corpus.fixture(fixture_id)
                key = (scenario.id, fixture.id)
                for implementation_id in eligible_by_key[key]:
                    command = adapter_command(
                        suite,
                        implementation_id,
                        scenario.id,
                        fixture.path,
                        fixture.password,
                        reference_python,
                        candidate_python,
                        timed=True,
                    )
                    outcome, wall_time_ns = measured_outcome(
                        run_adapter(command), scenario.id
                    )
                    scenario_timings.append(
                        benchmark_scenarios.build_scenario_sample(
                            scenario=scenario,
                            untimed_record=records_by_key[key][implementation_id],
                            measured_outcome=outcome,
                            wall_time_ns=wall_time_ns,
                            command_argv=recorded_command_argv(command),
                            repetition=(repetition if repetitions > 1 else None),
                        )
                    )
    return records, decisions, scenario_timings


def write_report(suite: benchmark_scenarios.ScenarioSuite) -> None:
    REPORT_PATH.parent.mkdir(parents=True, exist_ok=True)
    REPORT_PATH.write_text(
        benchmark_scenarios.render_markdown(suite),
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
            print(f"Scenario suite OK: {len(suite.scenarios)} workload scenarios")
            return 0
        if args.build:
            build_adapters()
            print(f"Built {suite.competitor_suite.rust_binary}")
            return 0

        assert args.output is not None
        records, decisions, scenario_timings = execute_local_run(
            suite,
            args.reference_python,
            args.candidate_python,
        )
        benchmark_scenarios.write_local_run(
            args.output,
            records=records,
            preflight_decisions=decisions,
            scenario_timings=scenario_timings,
        )
        print(
            f"Wrote local unpublished scenario run: {len(records)} records, "
            f"{len(scenario_timings)} eligible samples"
        )
        return 0
    except (
        benchmark_scenarios.BenchmarkScenarioError,
        OSError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"Scenario benchmark error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
