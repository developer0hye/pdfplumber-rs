"""Contract for the complete pinned object-schema snapshot (OBJ-009)."""

from __future__ import annotations

import hashlib
import json
import re
import unittest
from pathlib import Path
from typing import cast

from compat.harness import (
    corpus_index,
    lockfile,
    object_schema_snapshot,
    upstream,
)


EXPECTED_COLLECTION_COUNTS: dict[str, int] = {
    "external-parser": 28,
    "project-generated": 26,
    "rust-regression": 88,
    "upstream-v0.11.10": 81,
}
EXPECTED_OBJECT_TYPES: tuple[str, ...] = (
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


class CompleteObjectSchemaSnapshotTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.snapshot_path: Path = object_schema_snapshot.snapshot_path()
        cls.assertTrue(
            cls.snapshot_path.is_file(),
            "missing pinned object-schema snapshot: "
            f"{cls.snapshot_path.relative_to(upstream.REPO_ROOT)}",
        )
        cls.snapshot: dict[str, object] = json.loads(
            cls.snapshot_path.read_text(encoding="utf-8")
        )

    def test_snapshot_is_bound_to_target_environment_and_corpus(self) -> None:
        target: upstream.Target = upstream.load_target()
        self.assertEqual(self.snapshot["schema_version"], 1)
        self.assertEqual(
            self.snapshot["target"],
            {
                "project": target.project,
                "version": target.version,
                "tag": target.tag,
                "commit": target.commit,
                "repository": target.repository,
            },
        )
        self.assertEqual(
            self.snapshot["environment"],
            {
                "python_version": upstream.load_environment().python_version,
                "lockfile_sha256": lockfile.digest(),
            },
        )
        self.assertEqual(
            self.snapshot["configuration"],
            {
                "page_limit": 3,
                "laparams": {"detect_vertical": True},
            },
        )

        registry_path: Path = (
            upstream.REPO_ROOT / "compat" / "fixture-provenance.toml"
        )
        registry_digest: str = hashlib.sha256(registry_path.read_bytes()).hexdigest()
        self.assertEqual(
            self.snapshot["corpus"],
            {
                "fixture_count": 223,
                "registry": "compat/fixture-provenance.toml",
                "registry_sha256": registry_digest,
            },
        )

    def test_every_collection_attempt_and_failure_is_explicit(self) -> None:
        collections: dict[str, dict[str, object]] = cast(
            dict[str, dict[str, object]], self.snapshot["collections"]
        )
        self.assertEqual(set(collections), set(EXPECTED_COLLECTION_COUNTS))

        index: corpus_index.CorpusIndex = corpus_index.load_index(
            upstream.REPO_ROOT / "compat" / "fixture-provenance.toml"
        )
        indexed_paths: set[str] = {fixture.path for fixture in index.fixtures}
        for collection_id, expected_count in EXPECTED_COLLECTION_COUNTS.items():
            collection: dict[str, object] = collections[collection_id]
            failures: list[dict[str, str]] = cast(
                list[dict[str, str]], collection["failures"]
            )
            self.assertEqual(collection["fixture_count"], expected_count)
            self.assertEqual(
                cast(int, collection["completed_fixture_count"]) + len(failures),
                expected_count,
            )
            failure_paths: list[str] = [failure["path"] for failure in failures]
            self.assertEqual(failure_paths, sorted(failure_paths))
            self.assertEqual(len(failure_paths), len(set(failure_paths)))
            self.assertTrue(set(failure_paths).issubset(indexed_paths))
            for failure in failures:
                self.assertEqual(set(failure), {"error_type", "message", "path"})
                self.assertTrue(failure["error_type"])

        rust_failures: list[dict[str, str]] = cast(
            list[dict[str, str]], collections["rust-regression"]["failures"]
        )
        unicode_failure: dict[str, str] = next(
            failure
            for failure in rust_failures
            if failure["path"].endswith("annotations-unicode-issues.pdf")
        )
        self.assertIn("byte 0x80", unicode_failure["message"])

    def test_every_object_type_and_collection_has_a_schema_observation(self) -> None:
        object_types: dict[str, dict[str, object]] = cast(
            dict[str, dict[str, object]], self.snapshot["object_types"]
        )
        self.assertEqual(tuple(object_types), EXPECTED_OBJECT_TYPES)

        for object_type, record in object_types.items():
            schemas: list[list[str]] = cast(list[list[str]], record["schemas"])
            observations: dict[str, dict[str, int]] = cast(
                dict[str, dict[str, int]], record["collections"]
            )
            self.assertEqual(len(schemas), 1, object_type)
            self.assertEqual(set(observations), set(EXPECTED_COLLECTION_COUNTS))
            self.assertTrue(all(schemas[0]), object_type)
            self.assertEqual(len(schemas[0]), len(set(schemas[0])), object_type)
            self.assertGreater(
                sum(value["object_count"] for value in observations.values()),
                0,
                object_type,
            )
            for collection_id, observation in observations.items():
                self.assertEqual(
                    set(observation),
                    {"fixture_count", "object_count"},
                    f"{object_type}/{collection_id}",
                )
                self.assertGreaterEqual(observation["fixture_count"], 0)
                self.assertGreaterEqual(observation["object_count"], 0)

    def test_ordered_schemas_cover_each_upstream_dictionary_family(self) -> None:
        object_types: dict[str, dict[str, object]] = cast(
            dict[str, dict[str, object]], self.snapshot["object_types"]
        )

        self.assertEqual(
            object_types["char"]["schemas"],
            [
                [
                    "matrix",
                    "fontname",
                    "adv",
                    "upright",
                    "x0",
                    "y0",
                    "x1",
                    "y1",
                    "width",
                    "height",
                    "size",
                    "mcid",
                    "tag",
                    "object_type",
                    "page_number",
                    "ncs",
                    "text",
                    "stroking_color",
                    "non_stroking_color",
                    "top",
                    "bottom",
                    "doctop",
                ]
            ],
        )
        self.assertEqual(
            object_types["line"]["schemas"],
            [
                [
                    "x0",
                    "y0",
                    "x1",
                    "y1",
                    "width",
                    "height",
                    "pts",
                    "linewidth",
                    "stroke",
                    "fill",
                    "evenodd",
                    "stroking_color",
                    "non_stroking_color",
                    "mcid",
                    "tag",
                    "object_type",
                    "page_number",
                    "path",
                    "dash",
                    "top",
                    "bottom",
                    "doctop",
                ]
            ],
        )
        self.assertEqual(
            object_types["rect"]["schemas"], object_types["line"]["schemas"]
        )
        self.assertEqual(
            object_types["curve"]["schemas"], object_types["line"]["schemas"]
        )
        self.assertEqual(
            object_types["image"]["schemas"],
            [
                [
                    "x0",
                    "y0",
                    "x1",
                    "y1",
                    "width",
                    "height",
                    "name",
                    "stream",
                    "srcsize",
                    "imagemask",
                    "bits",
                    "colorspace",
                    "mcid",
                    "tag",
                    "object_type",
                    "page_number",
                    "top",
                    "bottom",
                    "doctop",
                ]
            ],
        )
        self.assertEqual(
            object_types["annot"]["schemas"],
            object_types["hyperlink"]["schemas"],
        )
        self.assertEqual(
            object_types["textboxhorizontal"]["schemas"],
            object_types["textboxvertical"]["schemas"],
        )
        self.assertEqual(
            object_types["textlinehorizontal"]["schemas"],
            object_types["textlinevertical"]["schemas"],
        )

        serialized: str = json.dumps(self.snapshot, sort_keys=True)
        self.assertNotIn(str(upstream.REPO_ROOT), serialized)
        self.assertIsNone(re.search(r"0x[0-9a-fA-F]{8,}", serialized))

    def test_candidate_projection_compares_schemas_and_collection_presence(
        self,
    ) -> None:
        snapshot: dict[str, object] = {
            "object_types": {
                "char": {
                    "schemas": [["text", "x0"]],
                    "collections": {
                        "external-parser": {
                            "fixture_count": 2,
                            "object_count": 7,
                        },
                        "project-generated": {
                            "fixture_count": 0,
                            "object_count": 0,
                        },
                    },
                }
            }
        }
        self.assertEqual(
            object_schema_snapshot.comparison_projection(snapshot),
            {
                "char": {
                    "schemas": [["text", "x0"]],
                    "collections": {
                        "external-parser": True,
                        "project-generated": False,
                    },
                }
            },
        )


if __name__ == "__main__":
    unittest.main()
