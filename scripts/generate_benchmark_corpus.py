#!/usr/bin/env python3
"""Validate and render the versioned SCORE-001 benchmark corpus."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT))

from compat.harness import benchmark_corpus

MANIFEST_PATH = REPO_ROOT / "benchmarks" / "corpus-v0.3.0.toml"
REGISTRY_PATH = REPO_ROOT / "compat" / "fixture-provenance.toml"
OUTPUT_PATH = REPO_ROOT / "docs" / "benchmarks" / "corpus-v0.3.0.md"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail if the generated report is missing or stale",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        corpus = benchmark_corpus.audit_repository(
            REPO_ROOT,
            MANIFEST_PATH,
            REGISTRY_PATH,
        )
        rendered = benchmark_corpus.render_markdown(corpus)
    except benchmark_corpus.BenchmarkCorpusError as error:
        print(f"Benchmark corpus error: {error}", file=sys.stderr)
        return 1

    if args.check:
        if not OUTPUT_PATH.is_file():
            print(f"Benchmark corpus report is missing: {OUTPUT_PATH}", file=sys.stderr)
            return 1
        if OUTPUT_PATH.read_text(encoding="utf-8") != rendered:
            print(f"Benchmark corpus report is stale: {OUTPUT_PATH}", file=sys.stderr)
            return 1
        print(
            f"Benchmark corpus OK: {len(corpus.fixtures)} PDFs, "
            f"{len(corpus.semantic_classes())} semantic classes"
        )
        return 0

    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT_PATH.write_text(rendered, encoding="utf-8")
    print(f"Wrote {OUTPUT_PATH.relative_to(REPO_ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
