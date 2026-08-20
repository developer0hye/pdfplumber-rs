#!/usr/bin/env python3
"""Verify that the `pdfplumber` on this interpreter is the expected one.

Run inside a compatibility environment before anything reads from it:

    .venv-reference/bin/python scripts/verify_compat_env.py --reference
    .venv-candidate/bin/python scripts/verify_compat_env.py --candidate

Exits non-zero with an explanatory message when the wrong package is importable,
which is the whole point: a parity run that silently compares upstream against
itself reports perfect agreement and proves nothing (PARITY-004).
"""

from __future__ import annotations

import argparse
import importlib
import sys
from pathlib import Path

REPO_ROOT: Path = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT))

from compat.harness import environment, upstream  # noqa: E402


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument(
        "--reference",
        action="store_true",
        help="require the pinned upstream Python pdfplumber",
    )
    group.add_argument(
        "--candidate",
        action="store_true",
        help="require this project's binding, not upstream",
    )
    parser.add_argument(
        "--expect-root",
        type=Path,
        default=None,
        help="also require the package to be imported from under this directory",
    )
    arguments = parser.parse_args()

    try:
        module = importlib.import_module("pdfplumber")
    except ImportError as error:
        print(f"cannot import pdfplumber: {error}", file=sys.stderr)
        return 1

    target: upstream.Target = upstream.load_target()
    try:
        if arguments.reference:
            environment.verify_reference(module, expected_root=arguments.expect_root)
        else:
            environment.verify_candidate(module, expected_root=arguments.expect_root)
    except environment.EnvironmentMismatch as mismatch:
        print(f"environment check failed: {mismatch}", file=sys.stderr)
        return 1

    role: str = "reference" if arguments.reference else "candidate"
    version: str = getattr(module, "__version__", "unknown")
    print(f"{role} environment OK: pdfplumber {version} at {module.__file__}")
    if arguments.reference:
        print(f"matches pinned target {target.project} {target.version} ({target.tag})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
