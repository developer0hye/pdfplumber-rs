#!/usr/bin/env python3
"""Emit one canonical benchmark outcome from pinned Python pdfplumber."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import pdfplumber

EXPECTED_VERSION = "0.11.10"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workload", choices=("document-open", "text"), required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--password")
    return parser.parse_args()


def run(args: argparse.Namespace) -> object:
    if pdfplumber.__version__ != EXPECTED_VERSION:
        raise RuntimeError(
            f"expected pdfplumber {EXPECTED_VERSION}, got {pdfplumber.__version__}"
        )
    with pdfplumber.open(
        args.fixture,
        password=args.password,
        unicode_norm=None,
        repair=False,
    ) as document:
        if args.workload == "document-open":
            return {"page_count": len(document.pages)}
        return [
            {
                "page_number": page.page_number,
                "text": page.extract_text(layout=False),
            }
            for page in document.pages
        ]


def main() -> int:
    args = parse_args()
    try:
        outcome = {"status": "success", "value": run(args)}
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
