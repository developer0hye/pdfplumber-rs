#!/usr/bin/env python3
"""Generate or verify the pinned Python object-schema snapshot."""

from __future__ import annotations

import argparse
import difflib
import importlib
import json
import os
import sys
from pathlib import Path

import pdfplumber

SCRIPT_DIR: Path = Path(os.path.abspath(__file__)).parent
REPO_ROOT: Path = SCRIPT_DIR.parent
sys.path.insert(0, str(REPO_ROOT))

environment = importlib.import_module("compat.harness.environment")
object_schema_snapshot = importlib.import_module(
    "compat.harness.object_schema_snapshot"
)


def render(snapshot: dict[str, object]) -> str:
    return json.dumps(snapshot, indent=2, ensure_ascii=False, sort_keys=True) + "\n"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--check",
        action="store_true",
        help="fail if the committed snapshot differs from pinned upstream",
    )
    mode.add_argument(
        "--candidate",
        action="store_true",
        help="compare candidate schemas and collection presence with the snapshot",
    )
    return parser.parse_args()


def main() -> int:
    args: argparse.Namespace = parse_args()
    try:
        if args.candidate:
            environment.verify_candidate(
                pdfplumber,
                expected_root=environment.CANDIDATE_VENV,
            )
        else:
            environment.verify_reference(
                pdfplumber,
                expected_root=environment.REFERENCE_VENV,
            )
    except environment.EnvironmentMismatch as mismatch:
        role: str = "candidate" if args.candidate else "reference"
        print(
            f"refusing to snapshot {role} object schemas: {mismatch}",
            file=sys.stderr,
        )
        if not args.candidate:
            print("Run: bash scripts/setup_golden_venv.sh", file=sys.stderr)
        return 1

    output_path: Path = object_schema_snapshot.snapshot_path()
    generated_snapshot: dict[str, object] = object_schema_snapshot.build(pdfplumber)
    generated: str = render(generated_snapshot)

    if args.candidate:
        if not output_path.is_file():
            print(
                "Object-schema snapshot is missing: "
                f"{output_path.relative_to(REPO_ROOT)}"
            )
            return 1
        committed_snapshot: dict[str, object] = json.loads(
            output_path.read_text(encoding="utf-8")
        )
        expected: str = render(
            object_schema_snapshot.comparison_projection(committed_snapshot)
        )
        actual: str = render(
            object_schema_snapshot.comparison_projection(generated_snapshot)
        )
        if expected == actual:
            print("Candidate object schemas match the pinned reference exactly")
            return 0
        print("Candidate object schemas differ from the pinned reference")
        return _print_diff(expected, actual)

    if args.check:
        if not output_path.is_file():
            print(
                "Object-schema snapshot is missing: "
                f"{output_path.relative_to(REPO_ROOT)}"
            )
            return 1
        committed: str = output_path.read_text(encoding="utf-8")
        if committed == generated:
            print(
                "Object-schema snapshot is current: "
                f"{output_path.relative_to(REPO_ROOT)}"
            )
            return 0

        print(
            "Object-schema snapshot is stale: "
            f"{output_path.relative_to(REPO_ROOT)}"
        )
        return _print_diff(committed, generated)

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(generated, encoding="utf-8")
    print(f"Wrote object-schema snapshot: {output_path.relative_to(REPO_ROOT)}")
    return 0


def _print_diff(expected: str, actual: str) -> int:
    diff: list[str] = list(
        difflib.unified_diff(
            expected.splitlines(),
            actual.splitlines(),
            fromfile="pinned-reference",
            tofile="current-run",
            lineterm="",
        )
    )
    for line in diff[:200]:
        print(line)
    if len(diff) > 200:
        print(f"... {len(diff) - 200} additional diff lines omitted")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
