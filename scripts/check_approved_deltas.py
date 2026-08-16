#!/usr/bin/env python3
"""Validate the approved-delta registry against the pinned upstream target."""

from __future__ import annotations

import sys
from pathlib import Path

REPO_ROOT: Path = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT))

from compat.harness import approved_deltas, upstream  # noqa: E402


def main() -> int:
    path = REPO_ROOT / "compat" / "approved_deltas.toml"
    try:
        registry = approved_deltas.load_registry(path)
        target = upstream.load_target()
        approved_deltas.validate_target(registry, target.version, target.commit)
    except approved_deltas.DeltaRegistryError as error:
        print(f"approved-delta registry invalid: {error}", file=sys.stderr)
        return 1
    print(
        f"Approved-delta registry OK: {len(registry.deltas)} entries for "
        f"{target.project} {target.version} ({target.commit})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
