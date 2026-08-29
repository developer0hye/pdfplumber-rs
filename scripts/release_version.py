#!/usr/bin/env python3
"""Resolve the repository's canonical workspace release identity."""

from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import tomllib

VERSION_PATTERN = re.compile(
    r"^(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)$"
)


class ReleaseVersionError(ValueError):
    """Raised when the workspace release identity is missing or invalid."""


@dataclass(frozen=True)
class ReleaseIdentity:
    """Paths and tag derived from the root workspace package version."""

    version: str
    tag: str
    release_notes: Path
    readiness: Path


def workspace_package(manifest: dict[str, Any], context: str) -> dict[str, Any]:
    workspace = manifest.get("workspace")
    if not isinstance(workspace, dict):
        raise ReleaseVersionError(f"{context} has no [workspace] table")
    package = workspace.get("package")
    if not isinstance(package, dict):
        raise ReleaseVersionError(f"{context} has no [workspace.package] table")
    return package


def workspace_version(manifest: dict[str, Any], context: str) -> str:
    """Return the strict X.Y.Z version from a parsed workspace manifest."""

    version = workspace_package(manifest, context).get("version")
    if not isinstance(version, str) or VERSION_PATTERN.fullmatch(version) is None:
        raise ReleaseVersionError(
            f"{context} [workspace.package].version must use strict X.Y.Z"
        )
    return version


def resolve_package_version(
    package: dict[str, Any],
    workspace: dict[str, Any],
    context: str,
    *,
    require_inheritance: bool = False,
) -> str:
    """Resolve a package version, optionally requiring workspace inheritance."""

    raw_version = package.get("version")
    if require_inheritance and raw_version != {"workspace": True}:
        raise ReleaseVersionError(f"{context} must use version.workspace = true")
    if raw_version == {"workspace": True}:
        raw_version = workspace.get("version")
    if (
        not isinstance(raw_version, str)
        or VERSION_PATTERN.fullmatch(raw_version) is None
    ):
        raise ReleaseVersionError(f"{context} version must use strict X.Y.Z")
    return raw_version


def load_release_identity(repo_root: Path) -> ReleaseIdentity:
    """Load the one release identity derived from the root Cargo manifest."""

    manifest_path = repo_root / "Cargo.toml"
    try:
        manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
        raise ReleaseVersionError(
            f"cannot read workspace manifest {manifest_path}: {error}"
        ) from error
    version = workspace_version(manifest, str(manifest_path))
    tag = f"v{version}"
    return ReleaseIdentity(
        version=version,
        tag=tag,
        release_notes=Path(f"docs/releases/{tag}.md"),
        readiness=Path(f"docs/readiness/{tag}.md"),
    )
