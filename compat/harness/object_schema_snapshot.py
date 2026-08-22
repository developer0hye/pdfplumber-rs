"""Deterministic object-dictionary schema snapshots for pinned fixtures."""

from __future__ import annotations

import hashlib
import re
import types
from collections import Counter, defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import cast

from compat.harness import corpus_index, lockfile, upstream


SCHEMA_VERSION: int = 1
PAGE_LIMIT: int = 3
LAPARAMS: dict[str, bool] = {"detect_vertical": True}
BASE_OBJECT_TYPES: tuple[str, ...] = (
    "annot",
    "char",
    "curve",
    "figure",
    "hyperlink",
    "image",
    "line",
    "rect",
    "textboxhorizontal",
    "textboxvertical",
    "textlinehorizontal",
    "textlinevertical",
)
REGISTRY_PATH: Path = upstream.REPO_ROOT / "compat" / "fixture-provenance.toml"
_MEMORY_ADDRESS: re.Pattern[str] = re.compile(r"0x[0-9a-fA-F]{8,}")


def snapshot_path() -> Path:
    target: upstream.Target = upstream.load_target()
    return (
        upstream.REPO_ROOT
        / "compat"
        / "snapshots"
        / f"{target.project}-v{target.version}-object-schemas.json"
    )


def build(pdfplumber_module: types.ModuleType) -> dict[str, object]:
    """Scan every indexed fixture and return a deterministic schema matrix."""
    target: upstream.Target = upstream.load_target()
    environment: upstream.Environment = upstream.load_environment()
    fixture_index: corpus_index.CorpusIndex = corpus_index.load_index(REGISTRY_PATH)
    collection_ids: tuple[str, ...] = tuple(
        collection.id for collection in fixture_index.collections
    )
    schemas_by_type: dict[str, set[tuple[str, ...]]] = defaultdict(set)
    observations_by_type: dict[str, dict[str, dict[str, int]]] = defaultdict(dict)
    collections: dict[str, dict[str, object]] = {}

    for collection_id in collection_ids:
        fixtures: tuple[corpus_index.Fixture, ...] = fixture_index.fixtures_for(
            collection_id
        )
        object_counts: Counter[str] = Counter()
        fixture_paths_by_type: dict[str, set[str]] = defaultdict(set)
        failures: list[dict[str, str]] = []
        completed_fixture_count: int = 0

        for fixture in fixtures:
            try:
                fixture_schemas, fixture_object_counts = _scan_fixture(
                    pdfplumber_module,
                    upstream.REPO_ROOT / fixture.path,
                )
            except Exception as error:
                failures.append(_snapshot_failure(fixture.path, error))
                continue

            completed_fixture_count += 1
            for object_type, schemas in fixture_schemas.items():
                schemas_by_type[object_type].update(schemas)
                fixture_paths_by_type[object_type].add(fixture.path)
            object_counts.update(fixture_object_counts)

        collections[collection_id] = {
            "fixture_count": len(fixtures),
            "completed_fixture_count": completed_fixture_count,
            "failures": sorted(failures, key=lambda failure: failure["path"]),
        }
        all_object_types: set[str] = set(BASE_OBJECT_TYPES) | set(schemas_by_type)
        for object_type in all_object_types:
            observations_by_type[object_type][collection_id] = {
                "fixture_count": len(fixture_paths_by_type[object_type]),
                "object_count": object_counts[object_type],
            }

    all_object_types = set(BASE_OBJECT_TYPES) | set(schemas_by_type)
    object_types: dict[str, dict[str, object]] = {}
    for object_type in sorted(all_object_types):
        observations: dict[str, dict[str, int]] = observations_by_type[object_type]
        object_types[object_type] = {
            "schemas": [
                list(schema)
                for schema in sorted(schemas_by_type[object_type])
            ],
            "collections": {
                collection_id: observations.get(
                    collection_id,
                    {"fixture_count": 0, "object_count": 0},
                )
                for collection_id in collection_ids
            },
        }

    return {
        "schema_version": SCHEMA_VERSION,
        "target": {
            "project": target.project,
            "version": target.version,
            "tag": target.tag,
            "commit": target.commit,
            "repository": target.repository,
        },
        "environment": {
            "python_version": environment.python_version,
            "lockfile_sha256": lockfile.digest(),
        },
        "configuration": {
            "page_limit": PAGE_LIMIT,
            "laparams": dict(LAPARAMS),
        },
        "corpus": {
            "fixture_count": len(fixture_index.fixtures),
            "registry": str(REGISTRY_PATH.relative_to(upstream.REPO_ROOT)),
            "registry_sha256": hashlib.sha256(
                REGISTRY_PATH.read_bytes()
            ).hexdigest(),
        },
        "collections": collections,
        "object_types": object_types,
    }


def comparison_projection(snapshot: dict[str, object]) -> dict[str, object]:
    """Keep observable schemas and family presence, excluding count noise."""
    raw_object_types: dict[str, dict[str, object]] = cast(
        dict[str, dict[str, object]], snapshot["object_types"]
    )
    projection: dict[str, object] = {}
    for object_type in sorted(raw_object_types):
        record: dict[str, object] = raw_object_types[object_type]
        observations: dict[str, dict[str, int]] = cast(
            dict[str, dict[str, int]], record["collections"]
        )
        projection[object_type] = {
            "schemas": record["schemas"],
            "collections": {
                collection_id: observation["object_count"] > 0
                for collection_id, observation in sorted(observations.items())
            },
        }
    return projection


def _scan_fixture(
    pdfplumber_module: types.ModuleType,
    fixture_path: Path,
) -> tuple[dict[str, set[tuple[str, ...]]], Counter[str]]:
    schemas_by_type: dict[str, set[tuple[str, ...]]] = defaultdict(set)
    object_counts: Counter[str] = Counter()

    with pdfplumber_module.open(
        fixture_path,
        laparams=dict(LAPARAMS),
    ) as document:
        for page in document.pages[:PAGE_LIMIT]:
            page_objects: dict[str, Sequence[Mapping[str, object]]] = dict(
                page.objects
            )
            page_objects["annot"] = page.annots
            page_objects["hyperlink"] = page.hyperlinks
            for object_type, objects in page_objects.items():
                for object_dictionary in objects:
                    schema: tuple[str, ...] = tuple(object_dictionary)
                    schemas_by_type[object_type].add(schema)
                    object_counts[object_type] += 1

    return schemas_by_type, object_counts


def _snapshot_failure(path: str, error: Exception) -> dict[str, str]:
    message: str = str(error).replace(str(upstream.REPO_ROOT), "<repo>")
    return {
        "path": path,
        "error_type": f"{type(error).__module__}.{type(error).__qualname__}",
        "message": _MEMORY_ADDRESS.sub("<address>", message),
    }
