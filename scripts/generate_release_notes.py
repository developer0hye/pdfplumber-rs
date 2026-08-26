#!/usr/bin/env python3
"""Generate or verify the versioned release notes from canonical sources."""

from __future__ import annotations

import argparse
import difflib
import sys
from pathlib import Path

from check_package_metadata import MetadataError, load_matrix, render_release_notes

REPO_ROOT = Path(__file__).resolve().parents[1]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail if the versioned release notes are missing or stale",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        matrix = load_matrix()
        output_path = REPO_ROOT / matrix["release_notes"]
        expected = render_release_notes(matrix)
    except MetadataError as error:
        print(f"release-note generation failed: {error}", file=sys.stderr)
        return 1

    if args.check:
        actual = (
            output_path.read_text(encoding="utf-8") if output_path.is_file() else ""
        )
        if actual != expected:
            difference = difflib.unified_diff(
                actual.splitlines(),
                expected.splitlines(),
                fromfile=str(output_path.relative_to(REPO_ROOT)),
                tofile="generated",
                lineterm="",
            )
            print("\n".join(difference), file=sys.stderr)
            return 1
        print(f"release notes are current: {output_path.relative_to(REPO_ROOT)}")
        return 0

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(expected, encoding="utf-8")
    print(f"wrote {output_path.relative_to(REPO_ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
