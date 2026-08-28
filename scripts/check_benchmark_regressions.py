#!/usr/bin/env python3
"""Validate or execute the versioned SCORE-013 regression-alert policy."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Sequence
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from compat.harness import benchmark_regressions

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


def load_policy() -> benchmark_regressions.RegressionPolicy:
    """Load and validate every inherited benchmark policy."""

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


def load_run(path: Path) -> dict[str, object]:
    """Load one complete provenance run from UTF-8 JSON."""

    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise benchmark_regressions.BenchmarkRegressionError(
            f"cannot read benchmark run: {path}"
        ) from error
    if not isinstance(value, dict):
        raise benchmark_regressions.BenchmarkRegressionError(
            f"benchmark run root must be an object: {path}"
        )
    return value


def write_decision(
    path: Path, decision: benchmark_regressions.RegressionDecision
) -> None:
    """Write one deterministic machine-readable decision."""

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        benchmark_regressions.serialize_decision(decision),
        encoding="utf-8",
    )


def parse_arguments(arguments: Sequence[str] | None = None) -> argparse.Namespace:
    """Parse check or paired-run comparison arguments."""

    parser = argparse.ArgumentParser(description=__doc__)
    operation = parser.add_mutually_exclusive_group(required=True)
    operation.add_argument("--check", action="store_true")
    operation.add_argument("--compare", action="store_true")
    parser.add_argument("--baseline-run", action="append", type=Path, default=[])
    parser.add_argument("--candidate-run", action="append", type=Path, default=[])
    parser.add_argument("--decision-output", type=Path)
    return parser.parse_args(arguments)


def main(arguments: Sequence[str] | None = None) -> int:
    """Validate source drift or compare exactly two runs per revision."""

    args = parse_arguments(arguments)
    try:
        policy = load_policy()
        if args.check:
            if args.baseline_run or args.candidate_run or args.decision_output:
                raise benchmark_regressions.BenchmarkRegressionError(
                    "--check does not accept run or decision paths"
                )
            expected = benchmark_regressions.render_policy(policy)
            try:
                current = DOCUMENT_PATH.read_text(encoding="utf-8")
            except OSError as error:
                raise benchmark_regressions.BenchmarkRegressionError(
                    f"cannot read rendered policy: {DOCUMENT_PATH}"
                ) from error
            if current != expected:
                raise benchmark_regressions.BenchmarkRegressionError(
                    "rendered benchmark regression policy is stale"
                )
            print(
                "Benchmark regression policy OK: "
                f"{policy.id}, {policy.runs_per_revision} runs/revision, "
                f"{policy.repetitions_per_run} repetitions/run"
            )
            return 0

        if args.decision_output is None:
            raise benchmark_regressions.BenchmarkRegressionError(
                "--compare requires --decision-output"
            )
        if len(args.baseline_run) != policy.runs_per_revision:
            raise benchmark_regressions.BenchmarkRegressionError(
                f"--compare requires {policy.runs_per_revision} --baseline-run paths"
            )
        if len(args.candidate_run) != policy.runs_per_revision:
            raise benchmark_regressions.BenchmarkRegressionError(
                f"--compare requires {policy.runs_per_revision} --candidate-run paths"
            )
        try:
            baseline_runs = [load_run(path) for path in args.baseline_run]
            candidate_runs = [load_run(path) for path in args.candidate_run]
        except benchmark_regressions.BenchmarkRegressionError as error:
            decision = benchmark_regressions.RegressionDecision(
                schema_version=1,
                status=benchmark_regressions.INCONCLUSIVE,
                policy_id=policy.id,
                baseline_revision="",
                candidate_revision="",
                control_scale=None,
                control_group_count=0,
                reasons=(str(error),),
                comparisons=(),
            )
        else:
            decision = benchmark_regressions.compare_runs(
                policy,
                baseline_runs,
                candidate_runs,
            )
        write_decision(args.decision_output, decision)
        print(
            f"Benchmark regression decision: {decision.status}; "
            f"{len(decision.comparisons)} target groups; "
            f"{decision.control_group_count} controls"
        )
        return 0 if decision.status == benchmark_regressions.PASS else 1
    except benchmark_regressions.BenchmarkRegressionError as error:
        print(f"Benchmark regression policy error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
