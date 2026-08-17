#!/usr/bin/env python3
"""Generate reference or isolated-candidate text/table option results."""

from __future__ import annotations

import argparse
import difflib
import json
import os
import sys
from pathlib import Path
from typing import Any

SCRIPT_DIR: Path = Path(os.path.abspath(__file__)).parent
REPO_ROOT: Path = SCRIPT_DIR.parent
sys.path.insert(0, str(REPO_ROOT))

from compat.harness import environment, option_matrix  # noqa: E402


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
        "--candidate-output",
        type=Path,
        help="write results from an isolated candidate package for parity_report.py",
    )
    return parser.parse_args()


def main(root_package: Any = None) -> int:
    args: argparse.Namespace = parse_args()
    if root_package is None:
        import pdfplumber as root_package

    try:
        if args.candidate_output is None:
            environment.verify_reference(root_package)
        else:
            environment.verify_candidate(root_package)
    except environment.EnvironmentMismatch as mismatch:
        print(f"refusing to generate option matrix: {mismatch}", file=sys.stderr)
        if args.candidate_output is None:
            print("Run: bash scripts/setup_golden_venv.sh", file=sys.stderr)
        else:
            print(
                "Run this command with .venv-candidate/bin/python",
                file=sys.stderr,
            )
        return 1

    output_path: Path = option_matrix.snapshot_path()
    snapshot: dict[str, object] = option_matrix.build(root_package)
    generated: str = render(snapshot)
    records: list[dict[str, object]] = snapshot["cases"]  # type: ignore[assignment]
    errors: list[str] = [
        str(record["id"]) for record in records if record["status"] != "ok"
    ]

    if args.candidate_output is not None:
        candidate_output: Path = args.candidate_output
        candidate_output.parent.mkdir(parents=True, exist_ok=True)
        candidate_output.write_text(generated, encoding="utf-8")
        print(
            f"Wrote candidate option results: {candidate_output} "
            f"({len(records)} cases)"
        )
        if errors:
            print(f"Candidate option cases failed: {', '.join(errors)}")
            return 1
        return 0

    if args.check:
        if not output_path.is_file():
            print(
                f"Option-matrix snapshot is missing: {output_path.relative_to(REPO_ROOT)}"
            )
            return 1
        committed: str = output_path.read_text(encoding="utf-8")
        if committed != generated:
            print(
                f"Option-matrix snapshot is stale: {output_path.relative_to(REPO_ROOT)}"
            )
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
        print(
            f"Option-matrix snapshot is current: {output_path.relative_to(REPO_ROOT)} "
            f"({len(records)} cases)"
        )
        if errors:
            print(f"Option-matrix cases failed: {', '.join(errors)}")
            return 1
        return 0

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(generated, encoding="utf-8")
    print(
        f"Wrote option-matrix snapshot: {output_path.relative_to(REPO_ROOT)} "
        f"({len(records)} cases)"
    )
    if errors:
        print(f"Option-matrix cases failed: {', '.join(errors)}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
