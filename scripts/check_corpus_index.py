#!/usr/bin/env python3
"""Validate and summarize the single compatibility PDF corpus index."""

from __future__ import annotations

import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT))

from compat.harness import corpus_index  # noqa: E402


def main() -> int:
    registry_path = REPO_ROOT / "compat" / "fixture-provenance.toml"
    try:
        index = corpus_index.audit_repository(REPO_ROOT, registry_path)
    except corpus_index.CorpusIndexError as error:
        print(f"Corpus index error: {error}", file=sys.stderr)
        return 1

    counts = ", ".join(
        f"{collection_id}={count}"
        for collection_id, count in index.collection_counts().items()
    )
    print(f"Corpus index OK: {len(index.fixtures)} PDFs ({counts})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
