#!/usr/bin/env python3
"""Generate or verify the pinned Python pdfplumber public-API snapshot."""

from __future__ import annotations

import argparse
import difflib
import json
import os
import sys
from pathlib import Path

import pdfplumber

SCRIPT_DIR: Path = Path(os.path.abspath(__file__)).parent
REPO_ROOT: Path = SCRIPT_DIR.parent
sys.path.insert(0, str(REPO_ROOT))

from compat.harness import api_snapshot, environment  # noqa: E402


def render(snapshot: dict[str, object]) -> str:
    return json.dumps(snapshot, indent=2, ensure_ascii=False, sort_keys=True) + "\n"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail if the committed snapshot differs from pinned upstream",
    )
    return parser.parse_args()


def main() -> int:
    args: argparse.Namespace = parse_args()
    try:
        environment.verify_reference(pdfplumber)
    except environment.EnvironmentMismatch as mismatch:
        print(f"refusing to snapshot API: {mismatch}", file=sys.stderr)
        print("Run: bash scripts/setup_golden_venv.sh", file=sys.stderr)
        return 1

    # `pdfplumber.cli.main` binds sys.argv[1:] as its default at import time.
    # Normalize it after parsing this script's flags so generation and --check
    # reflect the same upstream callable instead of their own CLI arguments.
    sys.argv[:] = [sys.argv[0]]

    output_path: Path = api_snapshot.snapshot_path()
    generated: str = render(api_snapshot.build(pdfplumber))

    if args.check:
        if not output_path.is_file():
            print(f"API snapshot is missing: {output_path.relative_to(REPO_ROOT)}")
            return 1
        committed: str = output_path.read_text(encoding="utf-8")
        if committed == generated:
            print(
                f"API snapshot is current: {output_path.relative_to(REPO_ROOT)}"
            )
            return 0

        print(f"API snapshot is stale: {output_path.relative_to(REPO_ROOT)}")
        diff: list[str] = list(
            difflib.unified_diff(
                committed.splitlines(),
                generated.splitlines(),
                fromfile="committed",
                tofile="generated",
                lineterm="",
            )
        )
        for line in diff[:200]:
            print(line)
        if len(diff) > 200:
            print(f"... {len(diff) - 200} additional diff lines omitted")
        return 1

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(generated, encoding="utf-8")
    print(f"Wrote API snapshot: {output_path.relative_to(REPO_ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
