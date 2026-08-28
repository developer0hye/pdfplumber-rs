#!/usr/bin/env python3
"""Withdraw an invalid benchmark bundle while retaining its tag and audit trail."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from compat.harness import benchmark_retention
from scripts.audit_benchmark_results import load_plan

EXPECTED_GITHUB_LOGIN = "developer0hye"
GITHUB_REPOSITORY = "developer0hye/pdfplumber-rs"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--decision", type=Path, required=True)
    parser.add_argument("--audit-evidence-url", required=True)
    parser.add_argument("--apply", action="store_true")
    return parser.parse_args()


def run(*arguments: str) -> str:
    completed = subprocess.run(
        list(arguments),
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout.strip()


def load_decision(path: Path) -> benchmark_retention.RetentionDecision:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise benchmark_retention.BenchmarkRetentionError(
            f"cannot read withdrawal decision: {path}"
        ) from error
    if not isinstance(value, dict):
        raise benchmark_retention.BenchmarkRetentionError(
            "withdrawal decision root must be an object"
        )
    return benchmark_retention.parse_decision(value)


def verify_github_identity() -> None:
    run("gh", "auth", "status")
    login = run("gh", "api", "user", "--jq", ".login")
    if login != EXPECTED_GITHUB_LOGIN:
        raise benchmark_retention.BenchmarkRetentionError(
            f"active GitHub login is {login!r}, expected {EXPECTED_GITHUB_LOGIN!r}"
        )


def apply_withdrawal(
    plan: benchmark_retention.RetentionPlan,
    decision: benchmark_retention.RetentionDecision,
    audit_evidence_url: str,
) -> None:
    verify_github_identity()
    tombstone = benchmark_retention.render_withdrawal_tombstone(
        plan,
        decision,
        audit_evidence_url=audit_evidence_url,
    )
    title = f"Benchmark results v{plan.publication_plan.release} (withdrawn)"
    with tempfile.TemporaryDirectory() as temporary_directory:
        directory = Path(temporary_directory)
        notes_path = directory / "withdrawal.md"
        notes_path.write_text(tombstone, encoding="utf-8")
        decision_path = directory / benchmark_retention.withdrawal_decision_asset_name(
            plan
        )
        decision_path.write_text(
            benchmark_retention.serialize_decision(decision),
            encoding="utf-8",
        )
        run(
            "gh",
            "release",
            "edit",
            plan.release_tag,
            "--repo",
            GITHUB_REPOSITORY,
            "--title",
            title,
            "--notes-file",
            str(notes_path),
            "--prerelease",
        )
        published_asset_names = set(
            run(
                "gh",
                "release",
                "view",
                plan.release_tag,
                "--repo",
                GITHUB_REPOSITORY,
                "--json",
                "assets",
                "--jq",
                ".assets[].name",
            ).splitlines()
        )
        for asset_name in benchmark_retention.withdrawal_asset_names(plan):
            if asset_name not in published_asset_names:
                continue
            run(
                "gh",
                "release",
                "delete-asset",
                plan.release_tag,
                asset_name,
                "--repo",
                GITHUB_REPOSITORY,
                "--yes",
            )
        run(
            "gh",
            "release",
            "upload",
            plan.release_tag,
            str(decision_path),
            "--repo",
            GITHUB_REPOSITORY,
            "--clobber",
        )
        remaining_assets = set(
            run(
                "gh",
                "release",
                "view",
                plan.release_tag,
                "--repo",
                GITHUB_REPOSITORY,
                "--json",
                "assets",
                "--jq",
                ".assets[].name",
            ).splitlines()
        )
        retained_result_assets = remaining_assets.intersection(
            benchmark_retention.withdrawal_asset_names(plan)
        )
        if retained_result_assets:
            raise benchmark_retention.BenchmarkRetentionError(
                "withdrawal left result assets published: "
                + ", ".join(sorted(retained_result_assets))
            )


def main() -> int:
    args = parse_args()
    try:
        plan = load_plan()
        decision = load_decision(args.decision)
        tombstone = benchmark_retention.render_withdrawal_tombstone(
            plan,
            decision,
            audit_evidence_url=args.audit_evidence_url,
        )
        if not args.apply:
            print(tombstone, end="")
            print("Dry run only; pass --apply after reviewing the decision.")
            return 0
        apply_withdrawal(plan, decision, args.audit_evidence_url)
        print(
            f"Withdrew three result assets from {plan.release_tag}; "
            "the Release tombstone, source tag, and decision remain"
        )
        return 0
    except (
        benchmark_retention.BenchmarkRetentionError,
        OSError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"Benchmark result withdrawal error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
