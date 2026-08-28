"""Contracts for the curated Rust Serde JSON compatibility policy (DX-006)."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
POLICY = REPO_ROOT / "docs/rust-serde-schema.md"
FIXTURE = REPO_ROOT / "crates/pdfplumber/tests/fixtures/serde-schema-v1.json"
REFERENCE = REPO_ROOT / "references/rust-serde-schema.md"
README = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
CRATE_DOCS = (REPO_ROOT / "crates/pdfplumber/src/lib.rs").read_text(encoding="utf-8")
MODELS = (REPO_ROOT / "crates/pdfplumber/src/models.rs").read_text(encoding="utf-8")
ROADMAP = (REPO_ROOT / "ROADMAP.md").read_text(encoding="utf-8")
CHANGELOG = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")

CURATED_MODELS = {
    "BBox",
    "Cell",
    "Char",
    "Color",
    "ColumnMode",
    "Curve",
    "DedupeOptions",
    "DocumentMetadata",
    "ExplicitLines",
    "ExtractOptions",
    "ExtractWarning",
    "ExtractWarningCode",
    "Line",
    "MetadataEntry",
    "MetadataReference",
    "MetadataValue",
    "Orientation",
    "RawDocumentMetadata",
    "Rect",
    "Strategy",
    "Table",
    "TableQuality",
    "TableSettings",
    "TextBlock",
    "TextDirection",
    "TextLine",
    "TextOptions",
    "UnicodeNorm",
    "Word",
    "WordOptions",
}


class RustSerdeSchemaContractTests(unittest.TestCase):
    def test_policy_defines_scope_and_incompatible_changes(self) -> None:
        self.assertTrue(POLICY.is_file())
        policy = POLICY.read_text(encoding="utf-8")
        normalized = " ".join(policy.split())
        self.assertIn("`pdfplumber::models`", policy)
        self.assertIn("`serde-json-v1`", policy)
        self.assertIn("`0.3.x`", policy)
        self.assertRegex(normalized, r"(?i)field rename.*incompatible")
        self.assertRegex(normalized, r"(?i)field type.*incompatible")
        self.assertRegex(normalized, r"(?i)enum.*encoding.*incompatible")
        self.assertRegex(normalized, r"(?i)object member order.*not.*guarantee")

    def test_policy_separates_raw_json_from_other_surfaces(self) -> None:
        policy = POLICY.read_text(encoding="utf-8")
        normalized = " ".join(policy.split())
        self.assertRegex(normalized, r"(?i)direct.*serde_json")
        self.assertRegex(normalized, r"(?i)does not add.*envelope")
        self.assertRegex(normalized, r"(?i)other Serde formats.*not covered")
        self.assertRegex(normalized, r"(?i)WebAssembly.*separate")

    def test_v1_fixture_covers_every_curated_model(self) -> None:
        self.assertTrue(FIXTURE.is_file())
        fixture = json.loads(FIXTURE.read_text(encoding="utf-8"))
        self.assertEqual(fixture["schema"], "pdfplumber-rs/serde-json-v1")
        self.assertEqual(fixture["crate_line"], "0.3.x")
        self.assertEqual(set(fixture["models"]), CURATED_MODELS)
        self.assertTrue(all(fixture["models"].values()))

    def test_primary_docs_reference_policy_and_roadmap_advances(self) -> None:
        for document in (README, CRATE_DOCS, MODELS):
            self.assertIn("rust-serde-schema", document)
        self.assertIn("serde-json-v1", CHANGELOG)
        self.assertNotIn("### Version Rust-native schemas", ROADMAP)
        self.assertIn("DX-015", ROADMAP)
        self.assertTrue(REFERENCE.is_file())
        reference = REFERENCE.read_text(encoding="utf-8")
        self.assertIn("Serde enum representations", reference)
        self.assertIn("Serde field attributes", reference)


if __name__ == "__main__":
    unittest.main()
