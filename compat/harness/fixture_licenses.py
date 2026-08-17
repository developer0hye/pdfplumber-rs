"""Audit committed PDF fixtures for provenance and redistribution metadata."""

from __future__ import annotations

import hashlib
import re
import subprocess
import tomllib
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Iterable, Mapping


SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")
REVISION_PATTERN = re.compile(r"^[0-9a-f]{40}$")
SOURCE_ID_PATTERN = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
SPDX_EXPRESSION_PATTERN = re.compile(
    r"^[A-Za-z0-9][A-Za-z0-9.+-]*"
    r"(?:\s+(?:AND|OR|WITH)\s+[A-Za-z0-9][A-Za-z0-9.+-]*)*$"
)
# SPDX syntax alone also accepts invented identifiers and LicenseRef values.
# Keep this policy deliberately review-bound: adding another fixture license
# requires a maintainer to verify its redistribution terms and extend this set.
APPROVED_REDISTRIBUTABLE_SPDX_EXPRESSIONS = frozenset(
    {
        "Apache-2.0",
        "GPL-2.0-only",
        "MIT",
    }
)


class FixtureMetadataError(ValueError):
    """Raised when fixture provenance metadata is unsafe or incomplete."""


@dataclass(frozen=True)
class AuditResult:
    fixture_count: int
    source_count: int


def load_registry(path: Path) -> dict[str, object]:
    """Load one TOML provenance registry."""

    with path.open("rb") as registry_file:
        return tomllib.load(registry_file)


def tracked_pdf_paths(repo_root: Path) -> set[str]:
    """Return repository-relative PDF paths tracked by Git."""

    process = subprocess.run(
        ["git", "ls-files", "-z", "--", "*.pdf"],
        cwd=repo_root,
        check=True,
        capture_output=True,
    )
    return {
        path.decode("utf-8")
        for path in process.stdout.split(b"\0")
        if path
    }


def audit_repository(repo_root: Path, registry_path: Path) -> AuditResult:
    """Validate the registry against every PDF committed in ``repo_root``."""

    return validate_registry(
        load_registry(registry_path),
        repo_root,
        tracked_pdf_paths(repo_root),
    )


def validate_registry(
    registry: Mapping[str, object],
    repo_root: Path,
    fixture_paths: Iterable[str],
) -> AuditResult:
    """Validate inventory coverage, digests, provenance, and license policy."""

    problems: list[str] = []
    schema = registry.get("schema")
    if not isinstance(schema, dict) or schema.get("version") != 2:
        problems.append("schema.version must be 2")

    raw_sources = registry.get("sources")
    if not isinstance(raw_sources, list) or not raw_sources:
        problems.append("sources must be a non-empty array")
        raw_sources = []
    sources: dict[str, dict[str, object]] = {}
    for index, raw_source in enumerate(raw_sources, start=1):
        if not isinstance(raw_source, dict):
            problems.append(f"source {index} must be a table")
            continue
        source_id = raw_source.get("id")
        if not isinstance(source_id, str) or not SOURCE_ID_PATTERN.fullmatch(
            source_id
        ):
            problems.append(f"source {index} has an invalid id")
            continue
        if source_id in sources:
            problems.append(f"duplicate source id: {source_id}")
            continue
        sources[source_id] = raw_source
        _validate_source(source_id, raw_source, repo_root, problems)

    raw_fixtures = registry.get("fixtures")
    if not isinstance(raw_fixtures, list) or not raw_fixtures:
        problems.append("fixtures must be a non-empty array")
        raw_fixtures = []

    expected_paths = set(fixture_paths)
    registered_paths: set[str] = set()
    for index, raw_fixture in enumerate(raw_fixtures, start=1):
        if not isinstance(raw_fixture, dict):
            problems.append(f"fixture {index} must be a table")
            continue
        path = raw_fixture.get("path")
        if not _is_safe_pdf_path(path):
            problems.append(f"unsafe fixture path: {path!r}")
            continue
        assert isinstance(path, str)
        if path in registered_paths:
            problems.append(f"duplicate fixture path: {path}")
            continue
        registered_paths.add(path)
        _validate_fixture(path, raw_fixture, sources, repo_root, problems)

    for path in sorted(expected_paths - registered_paths):
        problems.append(f"unregistered PDF: {path}")
    for path in sorted(registered_paths - expected_paths):
        problems.append(f"stale fixture entry: {path}")

    if problems:
        raise FixtureMetadataError("; ".join(problems))
    return AuditResult(
        fixture_count=len(registered_paths),
        source_count=len(sources),
    )


def _validate_source(
    source_id: str,
    source: Mapping[str, object],
    repo_root: Path,
    problems: list[str],
) -> None:
    kind = source.get("kind")
    if kind not in {"external", "generated"}:
        problems.append(f"source {source_id} has invalid kind: {kind!r}")
    repository = source.get("repository")
    if not isinstance(repository, str) or not repository:
        problems.append(f"source {source_id} has no repository")
    revision = source.get("revision")
    if kind == "external" and (
        not isinstance(revision, str)
        or not REVISION_PATTERN.fullmatch(revision)
    ):
        problems.append(
            f"external source {source_id} needs an immutable 40-character revision"
        )
    license_expression = source.get("license")
    if (
        not isinstance(license_expression, str)
        or not SPDX_EXPRESSION_PATTERN.fullmatch(license_expression)
        or license_expression.lower() in {"unknown", "proprietary"}
    ):
        problems.append(f"source {source_id} needs a valid SPDX license")
    elif license_expression not in APPROVED_REDISTRIBUTABLE_SPDX_EXPRESSIONS:
        problems.append(f"source {source_id} needs an approved SPDX license")
    license_evidence = source.get("license_evidence")
    if not isinstance(license_evidence, str) or not license_evidence:
        problems.append(f"source {source_id} needs license evidence")
    elif kind == "external" and isinstance(revision, str):
        if revision not in license_evidence:
            problems.append(
                f"source {source_id} license evidence is not revision-pinned"
            )
        if isinstance(repository, str) and not license_evidence.startswith(
            repository
        ):
            problems.append(
                f"source {source_id} license evidence is outside its repository"
            )
    elif kind == "generated" and not (repo_root / license_evidence).is_file():
        problems.append(
            f"source {source_id} license evidence does not exist: "
            f"{license_evidence}"
        )
    if source.get("public") is not True:
        problems.append(f"source {source_id} is not public")
    if source.get("redistribution") != "allowed":
        problems.append(f"source {source_id} does not allow redistribution")


def _validate_fixture(
    path: str,
    fixture: Mapping[str, object],
    sources: Mapping[str, Mapping[str, object]],
    repo_root: Path,
    problems: list[str],
) -> None:
    source_id = fixture.get("source")
    if not isinstance(source_id, str) or source_id not in sources:
        problems.append(f"fixture {path} has unknown source: {source_id!r}")
    source_path = fixture.get("source_path")
    if not isinstance(source_path, str) or not source_path:
        problems.append(f"fixture {path} has no source_path")
    elif (
        isinstance(source_id, str)
        and source_id in sources
        and sources[source_id].get("kind") == "external"
        and not _is_safe_source_path(source_path)
    ):
        problems.append(f"fixture {path} has unsafe source_path: {source_path}")
    digest = fixture.get("sha256")
    if not isinstance(digest, str) or not SHA256_PATTERN.fullmatch(digest):
        problems.append(f"fixture {path} has invalid SHA-256 metadata")
        return
    fixture_path = repo_root / path
    if not fixture_path.is_file():
        problems.append(f"fixture file does not exist: {path}")
        return
    actual_digest = hashlib.sha256(fixture_path.read_bytes()).hexdigest()
    if actual_digest != digest:
        problems.append(
            f"SHA-256 mismatch for {path}: expected {digest}, got {actual_digest}"
        )


def _is_safe_pdf_path(path: object) -> bool:
    if not isinstance(path, str) or not path.endswith(".pdf"):
        return False
    pure_path = PurePosixPath(path)
    return (
        not pure_path.is_absolute()
        and ".." not in pure_path.parts
        and str(pure_path) == path
    )


def _is_safe_source_path(path: str) -> bool:
    pure_path = PurePosixPath(path)
    return (
        not pure_path.is_absolute()
        and ".." not in pure_path.parts
        and str(pure_path) == path
        and not any(character in path for character in ("?", "#"))
    )
