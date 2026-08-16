#!/usr/bin/env python3
"""Compare pdfplumber-rs output against Python pdfplumber, fixture by fixture.

Reports, per PDF, how closely the Rust CLI matches Python pdfplumber on
characters (text and bounding boxes), words, page text, and lattice tables.
Use it to pick the next parity gap to close and to confirm a change moved the
numbers in the right direction.

Must run inside the pinned reference environment — see scripts/setup_golden_venv.sh.
The interpreter is checked on startup, because a report generated against the
wrong pdfplumber compares an implementation with itself and always looks perfect
(PARITY-004).

    .venv-reference/bin/python scripts/parity_report.py
    .venv-reference/bin/python scripts/parity_report.py --repo ../pdfplumber-rs-some-worktree
    .venv-reference/bin/python scripts/parity_report.py --json report.json
"""

import argparse
import json
import os
import subprocess
import sys
from collections import Counter
from typing import Any

import pdfplumber

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, REPO_ROOT)

from compat.harness import environment  # noqa: E402

FIXTURE_DIRS = ["tests/fixtures/generated", "tests/fixtures/downloaded", "crates/pdfplumber/tests/fixtures/pdfs"]

# Coordinates are compared at this tolerance, in points. Anything larger is a
# real disagreement about where the object sits, not float noise.
COORD_TOLERANCE = 0.05


def find_fixtures(fixture_root: str) -> list[str]:
    paths = []
    for rel_dir in FIXTURE_DIRS:
        directory = os.path.join(fixture_root, rel_dir)
        if not os.path.isdir(directory):
            continue
        for name in sorted(os.listdir(directory)):
            if name.endswith(".pdf"):
                paths.append(os.path.join(directory, name))
    return paths


def run_cli(repo: str, args: list[str]) -> Any:
    """Run the pdfplumber CLI in `repo` and parse its JSON output."""
    result = subprocess.run(
        ["cargo", "run", "-q", "-p", "pdfplumber-cli", "--", *args],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip()[-500:])
    return json.loads(result.stdout) if result.stdout.strip() else []


def close(a: float, b: float) -> bool:
    return abs(a - b) <= COORD_TOLERANCE


def key_of(obj: dict, with_box: bool = True) -> tuple:
    """A comparable identity for an object: its text and, optionally, its box."""
    if not with_box:
        return (obj["text"],)
    return (
        obj["text"],
        round(obj["x0"] / COORD_TOLERANCE),
        round(obj["top"] / COORD_TOLERANCE),
        round(obj["x1"] / COORD_TOLERANCE),
        round(obj["bottom"] / COORD_TOLERANCE),
    )


def multiset_ratio(expected: list[dict], actual: list[dict], with_box: bool) -> float:
    """Share of expected objects that also appear in actual, counting duplicates.

    Compared as multisets so one extra object near the top of a page does not
    shift every later object and read as a total mismatch.
    """
    total = max(len(expected), len(actual))
    if total == 0:
        return 1.0
    want = Counter(key_of(o, with_box) for o in expected)
    got = Counter(key_of(o, with_box) for o in actual)
    return sum((want & got).values()) / total


def compare_chars(expected: list[dict], actual: list[dict]) -> dict:
    """How far the characters agree, by text alone and by text plus position."""
    return {
        "count_expected": len(expected),
        "count_actual": len(actual),
        "text_ratio": multiset_ratio(expected, actual, with_box=False),
        "box_ratio": multiset_ratio(expected, actual, with_box=True),
    }


def compare_words(expected: list[dict], actual: list[dict]) -> dict:
    return {
        "count_expected": len(expected),
        "count_actual": len(actual),
        "ratio": multiset_ratio(expected, actual, with_box=True),
    }


def compare_text(expected: str, actual: str) -> dict:
    return {"equal": expected == actual, "len_expected": len(expected), "len_actual": len(actual)}


def compare_tables(expected: list, actual: list) -> dict:
    expected_cells = sum(len(row) for table in expected for row in table)
    actual_cells = sum(len(row) for table in actual for row in table)
    matching = 0
    for want_table, got_table in zip(expected, actual):
        for want_row, got_row in zip(want_table, got_table):
            for want_cell, got_cell in zip(want_row, got_row):
                if (want_cell or "") == (got_cell or ""):
                    matching += 1
    total_cells = max(expected_cells, actual_cells)
    return {
        "tables_expected": len(expected),
        "tables_actual": len(actual),
        "cells_expected": expected_cells,
        "cells_actual": actual_cells,
        # Agreeing that a page holds no tables is a match, not a miss.
        "cell_ratio": 1.0 if total_cells == 0 else matching / total_cells,
    }


def python_side(path: str) -> dict:
    with pdfplumber.open(path) as pdf:
        page = pdf.pages[0]
        return {
            "chars": [
                {k: c[k] for k in ("text", "x0", "top", "x1", "bottom")} for c in page.chars
            ],
            "words": [
                {k: w[k] for k in ("text", "x0", "top", "x1", "bottom")}
                for w in page.extract_words()
            ],
            "text": page.extract_text() or "",
            "tables": page.extract_tables(),
        }


def rust_side(repo: str, path: str) -> dict:
    page_args = ["--pages", "1", "--format", "json"]
    chars = run_cli(repo, ["chars", path, *page_args])
    words = run_cli(repo, ["words", path, *page_args])
    text = run_cli(repo, ["text", path, "--pages", "1", "--format", "json"])["text"]
    tables = [table["rows"] for table in run_cli(repo, ["tables", path, *page_args])]
    return {"chars": chars, "words": words, "text": text, "tables": tables}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default=REPO_ROOT, help="worktree to test (default: this repo)")
    parser.add_argument("--fixtures", default=REPO_ROOT, help="repo holding tests/fixtures")
    parser.add_argument("--json", help="also write the full report here")
    parser.add_argument("--only", help="substring filter on the fixture filename")
    args = parser.parse_args()

    try:
        environment.verify_reference(pdfplumber)
    except environment.EnvironmentMismatch as mismatch:
        print(f"refusing to report parity: {mismatch}", file=sys.stderr)
        print("Run: bash scripts/setup_golden_venv.sh", file=sys.stderr)
        return 1

    report = {}
    print(f"{'fixture':<34} {'chars':>14} {'words':>8} {'text':>6} {'tables':>8}")
    print("-" * 74)

    for path in find_fixtures(args.fixtures):
        name = os.path.basename(path)
        if args.only and args.only not in name:
            continue
        try:
            expected = python_side(path)
        except Exception as exc:  # noqa: BLE001 - report and continue
            print(f"{name:<34} python failed: {exc}")
            continue
        try:
            actual = rust_side(args.repo, path)
        except Exception as exc:  # noqa: BLE001 - report and continue
            print(f"{name:<34} rust failed: {exc}")
            continue

        entry = {
            "chars": compare_chars(expected["chars"], actual["chars"]),
            "words": compare_words(expected["words"], actual["words"]),
            "text": compare_text(expected["text"], actual["text"]),
            "tables": compare_tables(expected["tables"], actual["tables"]),
        }
        report[name] = entry
        print(
            f"{name:<34} "
            f"{entry['chars']['text_ratio']:>6.3f}/{entry['chars']['box_ratio']:<7.3f} "
            f"{entry['words']['ratio']:>8.3f} "
            f"{'yes' if entry['text']['equal'] else 'no':>6} "
            f"{entry['tables']['cell_ratio']:>8.3f}"
        )

    if report:
        print("-" * 74)
        print(
            "mean       "
            f"chars(text/box) {sum(e['chars']['text_ratio'] for e in report.values()) / len(report):.3f}"
            f"/{sum(e['chars']['box_ratio'] for e in report.values()) / len(report):.3f}  "
            f"words {sum(e['words']['ratio'] for e in report.values()) / len(report):.3f}  "
            f"text {sum(1 for e in report.values() if e['text']['equal'])}/{len(report)}  "
            f"tables {sum(e['tables']['cell_ratio'] for e in report.values()) / len(report):.3f}"
        )

    if args.json:
        with open(args.json, "w") as handle:
            json.dump(report, handle, indent=1)

    return 0


if __name__ == "__main__":
    sys.exit(main())
