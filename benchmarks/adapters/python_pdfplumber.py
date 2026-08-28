#!/usr/bin/env python3
"""Emit one canonical benchmark outcome from pinned Python pdfplumber."""

from __future__ import annotations

import argparse
import json
import time
from pathlib import Path

import pdfplumber

EXPECTED_VERSION = "0.11.10"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    operation = parser.add_mutually_exclusive_group(required=True)
    operation.add_argument("--workload", choices=("document-open", "text"))
    operation.add_argument(
        "--stage",
        choices=(
            "document-open",
            "page-materialization",
            "character-extraction",
            "word-grouping",
            "table-detection",
            "serialization",
            "language-boundary-conversion",
        ),
    )
    parser.add_argument("--timed", action="store_true")
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--password")
    args = parser.parse_args()
    if args.timed and args.stage is None:
        parser.error("--timed requires --stage")
    return args


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


def _page_characters(
    pages: list[object],
    characters_by_page: list[list[dict[str, object]]],
) -> list[dict[str, object]]:
    return [
        {
            "page_number": page.page_number,
            "chars": [character["text"] for character in characters],
        }
        for page, characters in zip(pages, characters_by_page, strict=True)
    ]


def _page_words(
    pages: list[object],
    words_by_page: list[list[dict[str, object]]],
) -> list[dict[str, object]]:
    return [
        {
            "page_number": page.page_number,
            "words": [
                {
                    "text": word["text"],
                    "x0": word["x0"],
                    "top": word["top"],
                    "x1": word["x1"],
                    "bottom": word["bottom"],
                }
                for word in words
            ],
        }
        for page, words in zip(pages, words_by_page, strict=True)
    ]


def _canonical_page_text(pages: list[object]) -> list[dict[str, object]]:
    return [
        {
            "page_number": page.page_number,
            "text": page.extract_text(layout=False),
        }
        for page in pages
    ]


def run_stage(args: argparse.Namespace) -> tuple[object, int | None]:
    """Run one component with setup outside the optional monotonic clock."""

    assert args.stage is not None
    document = None
    elapsed_ns: int | None = None
    try:
        if args.stage == "document-open":
            started_ns = time.perf_counter_ns() if args.timed else None
            document = pdfplumber.open(
                args.fixture,
                password=args.password,
                unicode_norm=None,
                repair=False,
            )
            if started_ns is not None:
                elapsed_ns = time.perf_counter_ns() - started_ns
            return {"page_count": len(document.pages)}, elapsed_ns

        document = pdfplumber.open(
            args.fixture,
            password=args.password,
            unicode_norm=None,
            repair=False,
        )
        if args.stage == "page-materialization":
            started_ns = time.perf_counter_ns() if args.timed else None
            pages = list(document.pages)
            if started_ns is not None:
                elapsed_ns = time.perf_counter_ns() - started_ns
            return [
                {"page_number": page.page_number} for page in pages
            ], elapsed_ns

        pages = list(document.pages)
        if args.stage == "character-extraction":
            started_ns = time.perf_counter_ns() if args.timed else None
            characters_by_page = [list(page.chars) for page in pages]
            if started_ns is not None:
                elapsed_ns = time.perf_counter_ns() - started_ns
            return _page_characters(pages, characters_by_page), elapsed_ns

        if args.stage == "language-boundary-conversion":
            characters_by_page = [list(page.chars) for page in pages]
            return _page_characters(pages, characters_by_page), None

        if args.stage == "word-grouping":
            for page in pages:
                _ = page.chars
            started_ns = time.perf_counter_ns() if args.timed else None
            words_by_page = [
                page.extract_words(
                    x_tolerance=3.0,
                    y_tolerance=3.0,
                    keep_blank_chars=False,
                    use_text_flow=False,
                    expand_ligatures=True,
                )
                for page in pages
            ]
            if started_ns is not None:
                elapsed_ns = time.perf_counter_ns() - started_ns
            return _page_words(pages, words_by_page), elapsed_ns

        if args.stage == "table-detection":
            for page in pages:
                _ = page.chars
                _ = page.edges
            settings = {
                "vertical_strategy": "lines",
                "horizontal_strategy": "lines",
                "snap_tolerance": 3.0,
                "join_tolerance": 3.0,
                "intersection_tolerance": 3.0,
                "min_words_vertical": 3,
                "min_words_horizontal": 1,
            }
            started_ns = time.perf_counter_ns() if args.timed else None
            tables_by_page = [page.extract_tables(settings) for page in pages]
            if started_ns is not None:
                elapsed_ns = time.perf_counter_ns() - started_ns
            return [
                {"page_number": page.page_number, "tables": tables}
                for page, tables in zip(pages, tables_by_page, strict=True)
            ], elapsed_ns

        if args.stage == "serialization":
            canonical_value = _canonical_page_text(pages)
            started_ns = time.perf_counter_ns() if args.timed else None
            serialized = json.dumps(
                canonical_value,
                allow_nan=False,
                ensure_ascii=False,
                separators=(",", ":"),
                sort_keys=True,
            )
            if started_ns is not None:
                elapsed_ns = time.perf_counter_ns() - started_ns
            return {
                "utf8": serialized,
                "utf8_bytes": len(serialized.encode("utf-8")),
            }, elapsed_ns

        raise RuntimeError(f"unsupported stage: {args.stage}")
    finally:
        if document is not None:
            document.close()


def main() -> int:
    args = parse_args()
    try:
        if args.stage is None:
            outcome = {"status": "success", "value": run(args)}
        else:
            value, elapsed_ns = run_stage(args)
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
