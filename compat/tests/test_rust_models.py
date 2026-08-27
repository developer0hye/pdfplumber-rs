"""Contracts for the curated Rust data-model surface (DX-005)."""

from __future__ import annotations

import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
CRATE_ROOT = (REPO_ROOT / "crates/pdfplumber/src/lib.rs").read_text(encoding="utf-8")
MODELS_SOURCE = REPO_ROOT / "crates/pdfplumber/src/models.rs"
MODEL_GUIDE = REPO_ROOT / "docs/rust-data-models.md"
README = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
ROADMAP = (REPO_ROOT / "ROADMAP.md").read_text(encoding="utf-8")
CHANGELOG = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
REFERENCE = REPO_ROOT / "references/rust-public-data-models.md"


class RustModelContractTests(unittest.TestCase):
    def test_facade_defines_one_curated_model_module(self) -> None:
        self.assertIn("pub mod models;", CRATE_ROOT)
        self.assertTrue(MODELS_SOURCE.is_file())
        source = MODELS_SOURCE.read_text(encoding="utf-8")
        for family in (
            "Char",
            "Word",
            "BBox",
            "Line",
            "Rect",
            "Curve",
            "Cell",
            "Table",
            "DocumentMetadata",
            "RawDocumentMetadata",
            "ExtractWarning",
            "ExtractWarningCode",
            "ExtractOptions",
            "WordOptions",
            "TextOptions",
            "TableSettings",
        ):
            self.assertIn(family, source)

    def test_stability_boundary_is_explicit_and_versioned(self) -> None:
        self.assertTrue(MODEL_GUIDE.is_file())
        guide = MODEL_GUIDE.read_text(encoding="utf-8")
        self.assertIn("`pdfplumber::models`", guide)
        self.assertIn("`0.3.x`", guide)
        self.assertRegex(guide, r"(?i)public field.*breaking")
        self.assertRegex(guide, r"(?i)enum variant.*breaking")
        self.assertIn("rust-serde-schema.md", guide)
        normalized = " ".join(guide.split())
        self.assertRegex(normalized, r"(?i)implements.*Serialize.*Deserialize")
        self.assertRegex(normalized, r"(?i)JSON field names.*enum encodings")

    def test_units_origins_and_indices_are_documented(self) -> None:
        guide = MODEL_GUIDE.read_text(encoding="utf-8")
        self.assertRegex(guide, r"(?i)72 points.*inch")
        self.assertRegex(guide, r"(?i)top-left.*x.*right.*y.*down")
        self.assertRegex(guide, r"(?i)page indices.*zero-based")
        self.assertIn("`doctop`", guide)
        self.assertRegex(guide, r"(?i)text-space.*advance")

    def test_collection_ordering_is_documented(self) -> None:
        guide = MODEL_GUIDE.read_text(encoding="utf-8")
        self.assertRegex(guide, r"(?i)content-stream encounter order")
        self.assertRegex(guide, r"(?i)row-major.*top-to-bottom.*left-to-right")
        self.assertRegex(guide, r"(?i)metadata.*source order")
        self.assertRegex(guide, r"(?i)warnings.*encounter order")

    def test_optional_fields_distinguish_absence_from_empty_values(self) -> None:
        guide = MODEL_GUIDE.read_text(encoding="utf-8")
        self.assertIn("`None`", guide)
        self.assertIn('`Some("")`', guide)
        self.assertIn("`resolution_error`", guide)
        self.assertIn("`collect_warnings`", guide)
        self.assertRegex(guide, r"(?i)option fields.*disabled.*unbounded")

    def test_primary_docs_and_roadmap_link_the_contract(self) -> None:
        for document in (README, CRATE_ROOT):
            self.assertIn("rust-data-models", document)
        self.assertIn("`pdfplumber::models`", CHANGELOG)
        self.assertNotIn("### Define stable public data models", ROADMAP)
        self.assertIn("DX-007", ROADMAP)
        self.assertTrue(REFERENCE.is_file())
        reference = REFERENCE.read_text(encoding="utf-8")
        self.assertIn("Cargo SemVer Compatibility", reference)
        self.assertIn("pdfplumber v0.11.10", reference)


if __name__ == "__main__":
    unittest.main()
