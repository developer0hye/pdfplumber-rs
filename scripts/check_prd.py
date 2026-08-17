#!/usr/bin/env python3
"""Validate the PRD master checklist and Evidence Ledger contracts."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT))

from compat.harness import prd_linter  # noqa: E402


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "path",
        nargs="?",
        type=Path,
        default=REPO_ROOT / "PRD.md",
        help="PRD document to validate (default: repository PRD.md)",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        result = prd_linter.lint_document(args.path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, prd_linter.PrdLintError) as error:
        print(f"PRD lint failed: {error}", file=sys.stderr)
        return 1

    print(
        f"PRD contract OK: {result.task_count} tasks, "
        f"{result.checked_count} checked, {result.evidence_count} evidence rows"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
