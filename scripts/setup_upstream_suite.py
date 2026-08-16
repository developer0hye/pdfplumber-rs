#!/usr/bin/env python3
"""Materialize the exact upstream v0.11.10 suite from a verified Git checkout."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

REPO_ROOT: Path = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT))

from compat.harness import upstream_suite  # noqa: E402


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--manifest",
        type=Path,
        default=REPO_ROOT / "compat" / "upstream-suite.toml",
    )
    parser.add_argument(
        "--destination",
        type=Path,
        default=REPO_ROOT / "compat" / "upstream-tests",
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
        help="verify the existing materialized suite without network access",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    config = upstream_suite.load_source_manifest(args.manifest)
    try:
        if args.check:
            upstream_suite.verify_materialized_suite(args.destination, config)
            print(
                "Upstream suite is current: "
                f"{args.destination.relative_to(REPO_ROOT)} "
                f"({config.tests_file_count} files, {config.tests_sha256})"
            )
            return 0

        source = args.source if args.source is not None else args.cache
        if args.source is None:
            prepare_cache(source, config)
        upstream_suite.materialize_suite(source, args.destination, config)
    except upstream_suite.SuiteSourceMismatch as mismatch:
        print(f"refusing upstream suite source: {mismatch}", file=sys.stderr)
        return 1

    print(
        f"Materialized {config.project} {config.version} suite at "
        f"{args.destination.relative_to(REPO_ROOT)}"
    )
    print(f"Verified commit {config.commit} and tests tree {config.tests_tree}")
    return 0


def prepare_cache(cache: Path, config: upstream_suite.SourceConfig) -> None:
    if cache.exists() and not (cache / ".git").is_dir():
        raise upstream_suite.SuiteSourceMismatch(
            f"managed cache exists but is not a Git checkout: {cache}"
        )
    if not cache.exists():
        cache.parent.mkdir(parents=True, exist_ok=True)
        _run(
            "git",
            "clone",
            "--filter=blob:none",
            "--depth",
            "1",
            "--branch",
            config.tag,
            config.repository,
            str(cache),
        )
    remote = _run("git", "-C", str(cache), "remote", "get-url", "origin").stdout
    if _normalized_repository(remote.strip()) != _normalized_repository(
        config.repository
    ):
        raise upstream_suite.SuiteSourceMismatch(
            f"managed cache origin {remote.strip()} does not match {config.repository}"
        )
    status = _run("git", "-C", str(cache), "status", "--porcelain").stdout
    if status.strip():
        raise upstream_suite.SuiteSourceMismatch(
            f"managed cache has local changes: {cache}"
        )
    _run("git", "-C", str(cache), "fetch", "--depth", "1", "origin", config.tag)
    _run("git", "-C", str(cache), "checkout", "--detach", config.commit)


def _normalized_repository(value: str) -> str:
    return value.removesuffix(".git").rstrip("/")


def _run(*command: str) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(
            command,
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        rendered = " ".join(command)
        raise upstream_suite.SuiteSourceMismatch(
            f"command failed while preparing upstream tests: {rendered}"
        ) from error


if __name__ == "__main__":
    raise SystemExit(main())
