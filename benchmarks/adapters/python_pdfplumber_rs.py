#!/usr/bin/env python3
"""Measure the installed candidate's native-to-Python object conversion."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import pdfplumber
from python_stage_metrics import PythonStageMetrics

EXPECTED_VERSION = "0.3.0"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    operation = parser.add_mutually_exclusive_group(required=True)
    operation.add_argument(
        "--stage",
        choices=("language-boundary-conversion",),
    )
    operation.add_argument("--scenario", choices=("cache-hit-characters",))
    parser.add_argument("--timed", action="store_true")
    parser.add_argument("--resources", action="store_true")
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--password")
    args = parser.parse_args()
    if args.timed and args.resources:
        parser.error("--timed and --resources are separate passes")
    return args


def run(
    args: argparse.Namespace,
) -> tuple[object, int | None, dict[str, object] | None]:
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
        if args.scenario == "cache-hit-characters":
            page = pages[0]
            _ = page.chars
            metrics = PythonStageMetrics(
                args.scenario,
                timed=args.timed,
                resources=False,
            )
            metrics.start()
            characters = page.chars
            elapsed_ns, _ = metrics.finish()
            value = [
                {
                    "page_number": page.page_number,
                    "chars": [character["text"] for character in characters],
                }
            ]
            return value, elapsed_ns, None

        assert args.stage is not None
        # Populate each native Page cache before the boundary clock. The timed
        # property access then performs only native Char to Python dict/list
        # conversion plus the canonical text projection.
        for page in pages:
            page.extract_text(layout=False)
        metrics = PythonStageMetrics(
            args.stage,
            timed=args.timed,
            resources=args.resources,
        )
        metrics.start()
        value = [
            {
                "page_number": page.page_number,
                "chars": [character["text"] for character in page.chars],
            }
            for page in pages
        ]
        elapsed_ns, resources = metrics.finish()
        return value, elapsed_ns, resources


def main() -> int:
    args = parse_args()
    try:
        value, elapsed_ns, resources = run(args)
        outcome = {"status": "success", "value": value}
        if elapsed_ns is not None:
            if args.scenario is not None:
                outcome["timing"] = {
                    "scenario_id": args.scenario,
                    "clock": "monotonic-wall",
                    "wall_time_ns": elapsed_ns,
                }
            else:
                outcome["timing"] = {
                    "stage_id": args.stage,
                    "clock": "monotonic-wall",
                    "wall_time_ns": elapsed_ns,
                }
        if resources is not None:
            outcome["resources"] = resources
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
