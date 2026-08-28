#!/usr/bin/env python3
"""Validate SCORE-009 policy or audit a completed exact-tag reproduction."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from compat.harness import benchmark_retention

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


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument("--check", action="store_true")
    modes.add_argument("--write-index", action="store_true")
    modes.add_argument("--audit", action="store_true")
    parser.add_argument("--published-dir", type=Path)
    parser.add_argument("--reproduced-run", type=Path)
    parser.add_argument("--decision-output", type=Path)
    parser.add_argument("--release-tag")
    parser.add_argument("--source-revision")
    return parser.parse_args()


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


def check_repository(plan: benchmark_retention.RetentionPlan) -> None:
    if INDEX_PATH.read_text(encoding="utf-8") != benchmark_retention.render_index(plan):
        raise benchmark_retention.BenchmarkRetentionError(
            f"benchmark result index is stale: {INDEX_PATH}"
        )
    workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
    required_workflow_fragments = (
        "schedule:",
        "workflow_dispatch:",
        "permissions:\n  contents: read",
        "ref: ${{ env.RELEASE_TAG }}",
        "gh release download",
        "run_benchmark_provenance.py --build",
        "run_benchmark_provenance.py --run",
        "audit_benchmark_results.py --audit",
        "retention-decision.json",
    )
    for fragment in required_workflow_fragments:
        if fragment not in workflow:
            raise benchmark_retention.BenchmarkRetentionError(
                f"benchmark result audit workflow lacks {fragment!r}"
            )
    for prohibited in (
        "contents: write",
        "release delete",
        "release delete-asset",
        "release edit",
    ):
        if prohibited in workflow:
            raise benchmark_retention.BenchmarkRetentionError(
                f"read-only benchmark audit workflow contains {prohibited!r}"
            )
    ci = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
        encoding="utf-8"
    )
    if "python scripts/audit_benchmark_results.py --check" not in ci:
        raise benchmark_retention.BenchmarkRetentionError(
            "Continuous Integration lacks the benchmark retention drift gate"
        )


def read_json_object(path: Path, context: str) -> dict[str, object]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise benchmark_retention.BenchmarkRetentionError(
            f"cannot read {context}: {path}"
        ) from error
    if not isinstance(value, dict):
        raise benchmark_retention.BenchmarkRetentionError(
            f"{context} root must be an object"
        )
    return value


def validate_arguments(args: argparse.Namespace) -> None:
    audit_arguments = (
        args.published_dir,
        args.reproduced_run,
        args.decision_output,
        args.release_tag,
        args.source_revision,
    )
    if args.audit != all(argument is not None for argument in audit_arguments):
        raise benchmark_retention.BenchmarkRetentionError(
            "--published-dir, --reproduced-run, --decision-output, --release-tag, "
            "and --source-revision are required exactly with --audit"
        )


def main() -> int:
    args = parse_args()
    try:
        validate_arguments(args)
        plan = load_plan()
        if args.write_index:
            INDEX_PATH.write_text(benchmark_retention.render_index(plan), encoding="utf-8")
            print(f"Wrote {INDEX_PATH.relative_to(REPO_ROOT)}")
            return 0
        if args.check:
            check_repository(plan)
            print(
                f"Benchmark result retention OK: {plan.release_tag}, "
                f"status {plan.status}"
            )
            return 0

        assert args.published_dir is not None
        assert args.reproduced_run is not None
        assert args.decision_output is not None
        assert args.release_tag is not None
        assert args.source_revision is not None
        if args.release_tag != plan.release_tag:
            raise benchmark_retention.BenchmarkRetentionError(
                "requested release tag does not match the retention registry"
            )
        if args.source_revision != plan.source_revision:
            raise benchmark_retention.BenchmarkRetentionError(
                "checked-out tag revision does not match the retention registry"
            )
        decision = benchmark_retention.audit_release_assets(
            plan,
            args.published_dir,
            read_json_object(args.reproduced_run, "reproduced benchmark run"),
        )
        args.decision_output.write_text(
            benchmark_retention.serialize_decision(decision),
            encoding="utf-8",
        )
        print(
            f"Benchmark result audit decision: {decision.status}; "
            f"{len(decision.reasons)} reason(s)"
        )
        return 0 if decision.status == "retain" else 3
    except (
        benchmark_retention.BenchmarkRetentionError,
        OSError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"Benchmark result retention error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
