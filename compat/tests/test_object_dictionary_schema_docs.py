"""Contracts for the complete object-dictionary schema guide (DOC-010)."""

from __future__ import annotations

import json
import re
import unittest
from pathlib import Path
from typing import cast

REPO_ROOT = Path(__file__).resolve().parents[2]
GUIDE_PATH = REPO_ROOT / "docs" / "object-dictionary-schemas.md"
SNAPSHOT_PATH = (
    REPO_ROOT
    / "compat"
    / "snapshots"
    / "pdfplumber-v0.11.10-object-schemas.json"
)

EXPECTED_OBJECT_TYPES = (
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

CURVE_EDGE_SCHEMA = (
    "object_type",
    "x0",
    "x1",
    "top",
    "doctop",
    "bottom",
    "width",
    "height",
    "orientation",
)


def schema_cell(fields: tuple[str, ...]) -> str:
    return " → ".join(f"`{field}`" for field in fields)


def compact(text: str) -> str:
    return " ".join(text.split())


class ObjectDictionarySchemaDocumentationContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.guide = (
            GUIDE_PATH.read_text(encoding="utf-8") if GUIDE_PATH.is_file() else ""
        )
        cls.compact_guide = compact(cls.guide)
        cls.snapshot = cast(
            dict[str, object],
            json.loads(SNAPSHOT_PATH.read_text(encoding="utf-8")),
        )

    def test_canonical_guide_is_linked_from_every_object_entry_point(self) -> None:
        with self.subTest(artifact="canonical guide"):
            self.assertTrue(GUIDE_PATH.is_file(), f"missing guide: {GUIDE_PATH}")

        links = {
            "README.md": (
                "[object-dictionary schema guide]"
                "(docs/object-dictionary-schemas.md)"
            ),
            "crates/pdfplumber-py/README.md": (
                "[object-dictionary schema guide]"
                "(../../docs/object-dictionary-schemas.md)"
            ),
            "crates/pdfplumber-wasm/README.md": (
                "[object-dictionary schema guide]"
                "(../../docs/object-dictionary-schemas.md)"
            ),
            "docs/faq.md": (
                "[object-dictionary schema guide]"
                "(object-dictionary-schemas.md)"
            ),
            "docs/pre-parity-python-migration.md": (
                "[object-dictionary schema guide]"
                "(object-dictionary-schemas.md)"
            ),
            "docs/python-migration.md": (
                "[object-dictionary schema guide]"
                "(object-dictionary-schemas.md)"
            ),
            "docs/rust-api.md": (
                "[object-dictionary schema guide]"
                "(object-dictionary-schemas.md)"
            ),
            "docs/rust-data-models.md": (
                "[object-dictionary schema guide]"
                "(object-dictionary-schemas.md)"
            ),
        }
        for relative, link in links.items():
            with self.subTest(document=relative):
                rendered = (REPO_ROOT / relative).read_text(encoding="utf-8")
                self.assertIn(link, rendered)

        changelog = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        with self.subTest(document="CHANGELOG.md"):
            self.assertRegex(
                changelog,
                r"(?im)^- \*\*Object dictionaries:\*\* .*schema guide",
            )

    def test_every_observed_family_has_its_exact_ordered_schema(self) -> None:
        raw_object_types = cast(
            dict[str, dict[str, object]], self.snapshot["object_types"]
        )
        self.assertEqual(tuple(raw_object_types), EXPECTED_OBJECT_TYPES)

        for family, record in raw_object_types.items():
            schemas = cast(list[list[str]], record["schemas"])
            self.assertEqual(len(schemas), 1, family)
            expected = f"| `{family}` | {schema_cell(tuple(schemas[0]))} |"
            with self.subTest(family=family):
                self.assertIn(expected, self.guide)

        for statement in (
            "twelve observed object families",
            "one invariant ordered key schema per observed family",
            "optional fields remain present with `None`",
            "insertion order is observable",
            "do not sort dictionary keys",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_field_meanings_and_derived_edge_schemas_are_explicit(self) -> None:
        object_types = cast(
            dict[str, dict[str, object]], self.snapshot["object_types"]
        )
        graphical_schema = tuple(
            cast(list[list[str]], object_types["line"]["schemas"])[0]
        )
        derived_schemas = {
            "line-derived": (*graphical_schema, "orientation"),
            "rectangle-derived": (*graphical_schema, "orientation"),
            "curve-derived": CURVE_EDGE_SCHEMA,
        }
        for family, schema in derived_schemas.items():
            with self.subTest(edge_family=family):
                self.assertIn(
                    f"| `{family}` | {schema_cell(schema)} |",
                    self.guide,
                )

        for statement in (
            "`x0`, `x1`, `top`, and `bottom` use top-origin page space",
            "`y0` and `y1` use bottom-origin page space",
            "`doctop` is measured from the top of the document",
            "`page_number` is one-based",
            "`matrix` is a six-value tuple",
            "`srcsize` is a `(width, height)` tuple",
            "`dash` is a `([dash_array], dash_phase)` tuple",
            "`path` retains drawing commands and control points",
            "`pts` retains top-origin points",
            "`mcid` and `tag` describe marked content",
            "`data` is the resolved annotation dictionary",
            "a hyperlink dictionary keeps `object_type == \"annot\"`",
            "`orientation` is `\"h\"`, `\"v\"`, or `None`",
            "`.edges` concatenates line-derived, rectangle-derived, and curve-derived edges",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_containers_serialization_and_examples_preserve_exact_boundaries(
        self,
    ) -> None:
        for example in (
            'laparams={"detect_vertical": True}',
            "page.objects",
            "page.chars",
            "page.annots",
            "page.hyperlinks",
            "page.rect_edges",
            "page.curve_edges",
            "page.edges",
            "page.horizontal_edges",
            "page.vertical_edges",
            "page.to_dict(object_types=[\"char\", \"annot\"])",
            "page.to_json(object_types=[\"char\", \"image\"])",
            "pdf.objects",
            "pdf.to_dict(object_types=[\"char\"])",
        ):
            with self.subTest(example=example):
                self.assertIn(example, self.guide)

        for statement in (
            "`.objects` omits empty families",
            "annotations and hyperlinks are not members of `.objects`",
            "derived edges are not members of `.objects`",
            "direct dictionaries retain tuples and opaque stream objects",
            "JSON serialization converts tuples to arrays",
            "`include_attrs` and `exclude_attrs` filter serialized output",
            "the figure family has no `.figures` accessor",
            "serializing a discovered figure through `to_dict` raises `AttributeError`",
            "word and search-result dictionaries do not have `object_type` or `page_number`",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

        self.assertGreaterEqual(self.guide.count("```python"), 4)

    def test_snapshot_scope_current_surfaces_sources_and_claim_boundary(self) -> None:
        for statement in (
            "223 indexed fixtures",
            "four collection classes",
            "the first three pages",
            "32 recorded failure observations",
            "`laparams={\"detect_vertical\": True}`",
            "`char` is missing `ncs` and adds `direction`",
            "`line` is missing nine pinned fields and adds `orientation`",
            "`rect` is missing six pinned fields",
            "`curve` is missing five pinned fields",
            "`image` is missing `stream`, `imagemask`, `mcid`, and `tag`",
            "the current Python adapter does not emit `figure`",
            "the current Python adapter does not expose derived-edge properties",
            "annotation key order matches while `data` remains incomplete",
            "Rust uses typed structs and idiomatic field names",
            "WebAssembly exposes only serialized `Char` values through `chars()`",
            "does not establish object-schema compatibility",
            "documentation is not compatibility evidence",
            "compat/snapshots/pdfplumber-v0.11.10-object-schemas.json",
            "python scripts/generate_object_schema_snapshot.py --check",
            "7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62",
            "f6f1ce3e0e546b854787aff946601af44fcc6f69",
            "286e7e158c12da8305520ecc1f550f3bd8f1a906",
            "2589a4a36278f5e04d9092ec1a27faf2c4336883",
            "93f5f3b83002deb04c7c8c7462b7dab1c4d245a7",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

        self.assertIsNotNone(
            re.search(
                r"(?s)## Current surface boundaries.*"
                r"## Validation and provenance.*"
                r"## Claim boundary",
                self.guide,
            )
        )


if __name__ == "__main__":
    unittest.main()
