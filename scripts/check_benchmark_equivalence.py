#!/usr/bin/env python3
"""Validate policy or preflight two untimed benchmark output records."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from compat.harness import benchmark_corpus, benchmark_equivalence

DEFAULT_POLICY = REPO_ROOT / "benchmarks" / "equivalence-v0.3.0.toml"
DEFAULT_CORPUS = REPO_ROOT / "benchmarks" / "corpus-v0.3.0.toml"
DEFAULT_REGISTRY = REPO_ROOT / "compat" / "fixture-provenance.toml"
DEFAULT_REPORT = REPO_ROOT / "docs" / "benchmarks" / "equivalence-v0.3.0.md"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--write-report", action="store_true")
    parser.add_argument("--reference", type=Path)
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--policy", type=Path, default=DEFAULT_POLICY)
    parser.add_argument("--corpus", type=Path, default=DEFAULT_CORPUS)
    parser.add_argument("--registry", type=Path, default=DEFAULT_REGISTRY)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    modes = sum(
        (
            args.check,
            args.write_report,
            args.reference is not None or args.candidate is not None,
        )
    )
    if modes != 1:
        print(
            "select exactly one mode: --check, --write-report, or both record paths",
            file=sys.stderr,
        )
        return 2
    if (args.reference is None) != (args.candidate is None):
        print("--reference and --candidate are required together", file=sys.stderr)
        return 2

    try:
        policy = benchmark_equivalence.audit_repository(
            REPO_ROOT,
            args.policy,
            args.corpus,
            args.registry,
        )
        if args.write_report:
            DEFAULT_REPORT.parent.mkdir(parents=True, exist_ok=True)
            DEFAULT_REPORT.write_text(
                benchmark_equivalence.render_markdown(policy),
                encoding="utf-8",
            )
            print(f"Wrote {DEFAULT_REPORT.relative_to(REPO_ROOT)}")
            return 0
        if args.check:
            expected = benchmark_equivalence.render_markdown(policy)
            if not DEFAULT_REPORT.is_file():
                raise benchmark_equivalence.BenchmarkEquivalenceError(
                    f"equivalence report is missing: {DEFAULT_REPORT}"
                )
            if DEFAULT_REPORT.read_text(encoding="utf-8") != expected:
                raise benchmark_equivalence.BenchmarkEquivalenceError(
                    f"equivalence report is stale: {DEFAULT_REPORT}"
                )
            print(f"Benchmark equivalence policy OK: {len(policy.workloads)} workloads")
            return 0

        assert args.reference is not None
        assert args.candidate is not None
        corpus = benchmark_corpus.audit_repository(
            REPO_ROOT,
            args.corpus,
            args.registry,
        )
        decision = benchmark_equivalence.preflight(
            benchmark_equivalence.load_record(args.reference),
            benchmark_equivalence.load_record(args.candidate),
            policy,
            corpus,
        )
    except benchmark_equivalence.BenchmarkEquivalenceError as error:
        print(f"Benchmark equivalence error: {error}", file=sys.stderr)
        return 2

    print(json.dumps(decision.to_dict(), indent=2, sort_keys=True))
    return 0 if decision.eligible_for_timing else 1


if __name__ == "__main__":
    raise SystemExit(main())
