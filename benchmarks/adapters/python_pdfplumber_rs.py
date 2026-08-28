#!/usr/bin/env python3
"""Measure the installed candidate's native-to-Python object conversion."""

from __future__ import annotations

import argparse
import json
import time
from pathlib import Path

import pdfplumber

EXPECTED_VERSION = "0.3.0"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--stage",
        choices=("language-boundary-conversion",),
        required=True,
    )
    parser.add_argument("--timed", action="store_true")
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--password")
    return parser.parse_args()


def run(args: argparse.Namespace) -> tuple[object, int | None]:
    actual_version = pdfplumber._native.__version__
    if actual_version != EXPECTED_VERSION:
        raise RuntimeError(
            f"expected pdfplumber-rs {EXPECTED_VERSION}, got {actual_version}"
        )
    with pdfplumber.open(
        args.fixture,
        password=args.password,
        unicode_norm=None,
        repair=False,
    ) as document:
        pages = list(document.pages)
        # Populate each native Page cache before the boundary clock. The timed
        # property access then performs only native Char to Python dict/list
        # conversion plus the canonical text projection.
        for page in pages:
            page.extract_text(layout=False)
        started_ns = time.perf_counter_ns() if args.timed else None
        value = [
            {
                "page_number": page.page_number,
                "chars": [character["text"] for character in page.chars],
            }
            for page in pages
        ]
        elapsed_ns = (
            time.perf_counter_ns() - started_ns if started_ns is not None else None
        )
        return value, elapsed_ns


def main() -> int:
    args = parse_args()
    try:
        value, elapsed_ns = run(args)
        outcome = {"status": "success", "value": value}
        if elapsed_ns is not None:
            outcome["timing"] = {
                "stage_id": args.stage,
                "clock": "monotonic-wall",
                "wall_time_ns": elapsed_ns,
            }
    except Exception as error:  # noqa: BLE001 - errors are benchmark outcomes
        outcome = {
            "status": "error",
            "error": {"kind": type(error).__name__, "message": str(error)},
        }
    print(
        json.dumps(
            outcome,
            allow_nan=False,
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
