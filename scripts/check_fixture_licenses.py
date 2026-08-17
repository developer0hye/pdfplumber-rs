#!/usr/bin/env python3
"""Audit every committed PDF's provenance and redistribution metadata."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT))

from compat.harness import fixture_licenses  # noqa: E402


def main() -> int:
    registry_path = REPO_ROOT / "compat" / "fixture-provenance.toml"
    try:
        result = fixture_licenses.audit_repository(REPO_ROOT, registry_path)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"Fixture metadata audit failed: {error}", file=sys.stderr)
        return 1

    print(
        "Fixture metadata OK: "
        f"{result.fixture_count} PDFs, {result.source_count} sources"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
