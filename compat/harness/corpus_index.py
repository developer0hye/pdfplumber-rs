"""Deterministic index over every licensed compatibility PDF fixture."""

from __future__ import annotations

import re
import tomllib
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Mapping

from compat.harness import fixture_licenses


COLLECTION_ID_PATTERN = re.compile(r"^[a-z0-9]+(?:-[a-z0-9.]+)*$")
SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")


class CorpusIndexError(ValueError):
    """The unified fixture index is incomplete or ambiguous."""


@dataclass(frozen=True)
class Collection:
    id: str
    description: str


@dataclass(frozen=True)
class Fixture:
    path: str
    sha256: str
    source: str
    source_path: str
    collection: str


@dataclass(frozen=True)
class CorpusIndex:
    collections: tuple[Collection, ...]
    fixtures: tuple[Fixture, ...]

    def fixture(self, path: str) -> Fixture:
        """Return the exact repository-relative fixture or fail loudly."""

        for fixture in self.fixtures:
            if fixture.path == path:
                return fixture
        raise CorpusIndexError(f"unknown fixture path: {path}")

    def fixtures_for(self, collection_id: str) -> tuple[Fixture, ...]:
        """Return one collection in deterministic repository-path order."""

        if collection_id not in {collection.id for collection in self.collections}:
            raise CorpusIndexError(f"unknown collection: {collection_id}")
        return tuple(
            fixture
            for fixture in self.fixtures
            if fixture.collection == collection_id
        )

    def collection_counts(self) -> dict[str, int]:
        """Return stable collection counts suitable for CI output."""

        return {
            collection.id: len(self.fixtures_for(collection.id))
            for collection in self.collections
        }


def load_index(path: Path) -> CorpusIndex:
    """Load and validate the single committed fixture registry."""

    try:
        with path.open("rb") as registry_file:
            registry = tomllib.load(registry_file)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise CorpusIndexError(f"cannot read corpus index: {path}") from error
    return validate_index(registry)


def audit_repository(repo_root: Path, registry_path: Path) -> CorpusIndex:
    """Cross-check the index against Git, file bytes, and license metadata."""

    try:
        license_result = fixture_licenses.audit_repository(
            repo_root, registry_path
        )
        index = load_index(registry_path)
    except fixture_licenses.FixtureMetadataError as error:
        raise CorpusIndexError(str(error)) from error
    if license_result.fixture_count != len(index.fixtures):
        raise CorpusIndexError(
            "license inventory and corpus index have different fixture counts"
        )
    return index


def validate_index(registry: Mapping[str, object]) -> CorpusIndex:
    """Validate unique collections and one classification per fixture path."""

    schema = registry.get("schema")
    if not isinstance(schema, dict) or schema.get("version") != 2:
        raise CorpusIndexError("schema.version must be 2")

    raw_sources = registry.get("sources")
    if not isinstance(raw_sources, list) or not raw_sources:
        raise CorpusIndexError("sources must be a non-empty array")
    source_ids: set[str] = set()
    for index, raw_source in enumerate(raw_sources, start=1):
        if not isinstance(raw_source, dict):
            raise CorpusIndexError(f"source {index} must be a table")
        source_id = raw_source.get("id")
        if not isinstance(source_id, str) or not source_id:
            raise CorpusIndexError(f"source {index} has an invalid id")
        if source_id in source_ids:
            raise CorpusIndexError(f"duplicate source id: {source_id}")
        source_ids.add(source_id)

    raw_collections = registry.get("collections")
    if not isinstance(raw_collections, list) or not raw_collections:
        raise CorpusIndexError("collections must be a non-empty array")
    collection_by_id: dict[str, Collection] = {}
    for index, raw_collection in enumerate(raw_collections, start=1):
        if not isinstance(raw_collection, dict):
            raise CorpusIndexError(f"collection {index} must be a table")
        collection_id = raw_collection.get("id")
        description = raw_collection.get("description")
        if (
            not isinstance(collection_id, str)
            or not COLLECTION_ID_PATTERN.fullmatch(collection_id)
        ):
            raise CorpusIndexError(f"collection {index} has an invalid id")
        if collection_id in collection_by_id:
            raise CorpusIndexError(
                f"duplicate collection id: {collection_id}"
            )
        if not isinstance(description, str) or not description:
            raise CorpusIndexError(
                f"collection {collection_id} needs a description"
            )
        collection_by_id[collection_id] = Collection(
            id=collection_id,
            description=description,
        )

    raw_fixtures = registry.get("fixtures")
    if not isinstance(raw_fixtures, list) or not raw_fixtures:
        raise CorpusIndexError("fixtures must be a non-empty array")
    fixture_by_path: dict[str, Fixture] = {}
    for index, raw_fixture in enumerate(raw_fixtures, start=1):
        if not isinstance(raw_fixture, dict):
            raise CorpusIndexError(f"fixture {index} must be a table")
        path = raw_fixture.get("path")
        if not _is_safe_pdf_path(path):
            raise CorpusIndexError(f"unsafe fixture path: {path!r}")
        assert isinstance(path, str)
        if path in fixture_by_path:
            raise CorpusIndexError(f"duplicate fixture path: {path}")
        digest = raw_fixture.get("sha256")
        if not isinstance(digest, str) or not SHA256_PATTERN.fullmatch(digest):
            raise CorpusIndexError(f"fixture {path} has invalid SHA-256")
        source = raw_fixture.get("source")
        if not isinstance(source, str) or source not in source_ids:
            raise CorpusIndexError(
                f"fixture {path} has unknown source: {source!r}"
            )
        source_path = raw_fixture.get("source_path")
        if not isinstance(source_path, str) or not source_path:
            raise CorpusIndexError(f"fixture {path} has no source_path")
        collection = raw_fixture.get("collection")
        if not isinstance(collection, str) or not collection:
            raise CorpusIndexError(
                f"fixture {path} needs one non-empty collection"
            )
        if collection not in collection_by_id:
            raise CorpusIndexError(
                f"fixture {path} has unknown collection: {collection}"
            )
        fixture_by_path[path] = Fixture(
            path=path,
            sha256=digest,
            source=source,
            source_path=source_path,
            collection=collection,
        )

    return CorpusIndex(
        collections=tuple(
            collection_by_id[collection_id]
            for collection_id in sorted(collection_by_id)
        ),
        fixtures=tuple(
            fixture_by_path[path] for path in sorted(fixture_by_path)
        ),
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
