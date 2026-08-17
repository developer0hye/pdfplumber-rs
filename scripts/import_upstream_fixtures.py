#!/usr/bin/env python3
"""Import or verify the exact pinned upstream PDF fixture corpus."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT))

from compat.harness import upstream_fixture_corpus  # noqa: E402


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--manifest",
        type=Path,
        default=REPO_ROOT / "compat" / "upstream-fixtures.toml",
    )
    parser.add_argument(
        "--cache",
        type=Path,
        default=REPO_ROOT / "target" / "compat-upstream-source" / "pdfplumber",
    )
    parser.add_argument(
        "--source",
        type=Path,
        help="use an existing exact Git checkout instead of the managed cache",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the committed import without network access",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        config = upstream_fixture_corpus.load_manifest(args.manifest)
        if args.check:
            result = upstream_fixture_corpus.verify_import(REPO_ROOT, config)
            print(
                f"Upstream fixture corpus is current: {result.file_count} PDFs, "
                f"{result.sha256}"
            )
            return 0

        source = args.source if args.source is not None else args.cache
        if args.source is None:
            upstream_fixture_corpus.prepare_cache(source, config)
        result = upstream_fixture_corpus.materialize_corpus(
            source, REPO_ROOT, config
        )
    except upstream_fixture_corpus.CorpusMismatch as mismatch:
        print(f"refusing upstream fixture corpus: {mismatch}", file=sys.stderr)
        return 1

    print(
        f"Imported {config.project} {config.version} PDF fixtures: "
        f"{result.file_count} files, {result.sha256}"
    )
    print(f"Verified source commit {config.commit}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
