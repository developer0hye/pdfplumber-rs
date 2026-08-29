"""Contracts for crop and derived-page semantics documentation (DOC-007)."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GUIDE_PATH = REPO_ROOT / "docs" / "crop-semantics.md"


def compact(text: str) -> str:
    return " ".join(text.split())


class CropSemanticsDocumentationContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.guide = (
            GUIDE_PATH.read_text(encoding="utf-8") if GUIDE_PATH.is_file() else ""
        )
        cls.compact_guide = compact(cls.guide)

    def test_canonical_guide_is_linked_and_diagrams_both_coordinate_models(
        self,
    ) -> None:
        with self.subTest(artifact="canonical guide"):
            self.assertTrue(GUIDE_PATH.is_file(), f"missing guide: {GUIDE_PATH}")

        links = {
            "README.md": "[crop-semantics guide](docs/crop-semantics.md)",
            "crates/pdfplumber-py/README.md": (
                "[crop-semantics guide](../../docs/crop-semantics.md)"
            ),
            "crates/pdfplumber-wasm/README.md": (
                "[crop-semantics guide](../../docs/crop-semantics.md)"
            ),
            "docs/coordinate-systems.md": "[crop-semantics guide](crop-semantics.md)",
            "docs/faq.md": "[crop-semantics guide](crop-semantics.md)",
            "docs/pre-parity-python-migration.md": (
                "[crop-semantics guide](crop-semantics.md)"
            ),
            "docs/rust-api.md": "[crop-semantics guide](crop-semantics.md)",
        }
        for relative, link in links.items():
            with self.subTest(document=relative):
                rendered = (REPO_ROOT / relative).read_text(encoding="utf-8")
                self.assertIn(link, rendered)

        changelog = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        with self.subTest(document="CHANGELOG.md"):
            self.assertRegex(
                changelog,
                r"(?im)^- \*\*Crop semantics:\*\* .*crop-semantics guide",
            )

        with self.subTest(artifact="Mermaid diagrams"):
            self.assertGreaterEqual(self.guide.count("```mermaid"), 2)
        for label in (
            "Python pdfplumber crop",
            "Legacy Rust crop",
            "Root page coordinate frame",
            "Rebased crop coordinate frame",
        ):
            with self.subTest(diagram_label=label):
                self.assertIn(label, self.guide)

    def test_pinned_python_inclusion_clipping_and_bbox_rules_are_exact(self) -> None:
        for statement in (
            "`crop(bbox, relative=False, strict=True)`",
            "`crop` retains every object with an overlap",
            "clips the copied object's `x0`, `top`, `x1`, and `bottom` to the overlap",
            "recomputes `width` and `height`",
            "adjusts `doctop` only when clipping moves the object's `top`",
            "does not promise to clip nested geometry such as curve `pts` or `path`",
            "`within_bbox` retains only objects whose complete bounding box is inside the requested box",
            "`outside_bbox` retains only objects with no overlap",
            "`crop` and `within_bbox` set the derived page's `bbox` to the requested box",
            "`outside_bbox` preserves the immediate parent page's `bbox`",
            "touching at one point is not an overlap",
            "a zero-width or zero-height edge intersection can still be an overlap when the other dimension is positive",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_relative_strict_nested_and_root_coordinate_behavior_is_explicit(
        self,
    ) -> None:
        for statement in (
            "Pinned Python object coordinates remain in the root page's displayed coordinate frame",
            "clipping is not rebasing",
            "With `relative=False`, a nested operation still receives root-page coordinates",
            "With `relative=True`, `bbox.x0` and `bbox.top` are offsets from the immediate parent page's `bbox` origin",
            "strict validation occurs after resolving a relative box",
            "`strict=True` rejects zero-area boxes",
            "rejects boxes entirely outside the parent",
            "rejects boxes not fully within the parent",
            "`strict=False` skips those parent-boundary checks",
            "a crop outside the parent can therefore produce an empty derived view with the requested `bbox`",
            "`root_page` remains the original page and `parent_page` remains the immediate source view",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_current_rust_python_and_wasm_surfaces_are_not_conflated(self) -> None:
        for statement in (
            "The legacy behavior has not been removed from the current Rust API",
            "Rust `Page::crop` and `CroppedPage::crop` retain an object when its center lies inside the box",
            "They do not clip partially intersecting geometry",
            "subtract `bbox.x0` and `bbox.top` from retained coordinates",
            "return `bbox() == (0, 0, width, height)`",
            "Rust `within_bbox` and `outside_bbox` also rebase retained coordinates",
            "Rust `outside_bbox` uses the exclusion box's dimensions for the returned view",
            "The Python compatibility facade's `.objects`-backed collections apply overlap clipping in root coordinates",
            "`extract_text`, `extract_words`, and table methods still use the legacy Rust inner view",
            "The current Python methods accept only `bbox`; they do not expose `relative` or `strict`",
            "WebAssembly exposes no crop, region-filter, or `CroppedPage` API",
            "a method with the same name does not establish compatible crop behavior",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_migration_names_sources_and_claim_limits_are_unambiguous(self) -> None:
        for field_name in (
            "bbox_root_top_left_points",
            "bbox_view_top_left_points",
            "crop_bbox_root_top_left_points",
            "crop_bbox_relative_to_parent_top_left_points",
        ):
            with self.subTest(field_name=field_name):
                self.assertIn(field_name, self.guide)

        for rule in (
            "Do not compare, join, or serialize root-preserved and rebased coordinates under one field name",
            "The versioned compatibility scorecard marks the crop workflow `Not tested`",
            "crop documentation is not compatibility evidence",
            "does not approve a compatibility deviation",
            "REGION-001 through REGION-024 remain open",
        ):
            with self.subTest(rule=rule):
                self.assertIn(rule, self.compact_guide)

        for source in (
            "https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/page.py",
            "https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/utils/geometry.py",
            "../crates/pdfplumber/src/cropped_page.rs",
            "../crates/pdfplumber-py/src/lib.rs",
            "compatibility/workflows-v0.3.0.md#crop",
        ):
            with self.subTest(source=source):
                self.assertIn(source, self.guide)

        self.assertNotRegex(self.guide, re.compile(r"\b\d+(?:\.\d+)?%\b"))


if __name__ == "__main__":
    unittest.main()
