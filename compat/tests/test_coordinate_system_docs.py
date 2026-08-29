"""Contracts for coordinate-system and page-box documentation (DOC-006)."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GUIDE_PATH = REPO_ROOT / "docs" / "coordinate-systems.md"


def compact(text: str) -> str:
    return " ".join(text.split())


class CoordinateSystemDocumentationContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.guide = (
            GUIDE_PATH.read_text(encoding="utf-8") if GUIDE_PATH.is_file() else ""
        )
        cls.compact_guide = compact(cls.guide)

    def test_canonical_guide_is_linked_and_contains_three_diagrams(self) -> None:
        with self.subTest(artifact="canonical guide"):
            self.assertTrue(GUIDE_PATH.is_file(), f"missing guide: {GUIDE_PATH}")

        links = {
            "README.md": "[coordinate-system guide](docs/coordinate-systems.md)",
            "crates/pdfplumber-py/README.md": (
                "[coordinate-system guide](../../docs/coordinate-systems.md)"
            ),
            "crates/pdfplumber-wasm/README.md": (
                "[coordinate-system guide](../../docs/coordinate-systems.md)"
            ),
            "docs/faq.md": "[coordinate-system guide](coordinate-systems.md)",
            "docs/rust-api.md": "[coordinate-system guide](coordinate-systems.md)",
            "docs/rust-data-models.md": (
                "[coordinate-system guide](coordinate-systems.md)"
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
                r"(?im)^- \*\*Coordinate systems:\*\* .*coordinate-system guide",
            )

        with self.subTest(artifact="Mermaid diagrams"):
            self.assertGreaterEqual(self.guide.count("```mermaid"), 3)
        for label in (
            "PDF native user space",
            "Displayed page space",
            "Source page boxes",
            "Document top",
        ):
            with self.subTest(diagram_label=label):
                self.assertIn(label, self.guide)

    def test_units_axes_rotation_and_conversion_formulas_are_explicit(self) -> None:
        for statement in (
            "One PDF point is nominally 1/72 inch",
            "`x` increases to the right in every documented public surface",
            "Native PDF user space normally places the origin at the bottom-left and increases `y` upward",
            "Displayed page space places the origin at the top-left and increases `y` downward",
            "`BBox` means `(x0, top, x1, bottom)` in displayed object geometry",
            "`width = x1 - x0`",
            "`height = bottom - top`",
            "`top = mediabox_top + page_height - y1`",
            "`bottom = mediabox_top + page_height - y0`",
            "`/Rotate` is clockwise and is normalized to `0`, `90`, `180`, or `270` degrees",
            "A `90`- or `270`-degree rotation swaps displayed width and height",
            "Do not apply rotation or the vertical inversion a second time",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_page_boxes_and_surface_representations_stay_distinct(self) -> None:
        for statement in (
            "MediaBox defines the physical page boundary",
            "CropBox defines the intended visible or clipped region",
            "TrimBox defines the intended finished-page boundary",
            "BleedBox defines the production bleed boundary",
            "ArtBox defines the extent of meaningful page content",
            "the boxes are not guaranteed to be concentric or strictly nested",
            "Rust source page-box getters preserve PDF array order `[x0, y0, x1, y1]` inside `BBox` fields",
            "`Page::bbox()` is the displayed page rectangle `(0, 0, width, height)`",
            "Rust uses the MediaBox, not the CropBox, for page dimensions and object coordinate transforms",
            "`Page.cropbox` falls back to `Page.mediabox` when the PDF omits CropBox",
            "`Page.bbox` initially equals `Page.mediabox`",
            "WebAssembly does not expose page-box getters",
            "Crop inclusion, clipping, and rebasing semantics belong to DOC-007",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_object_page_and_document_vertical_coordinates_are_mapped(self) -> None:
        for statement in (
            "Python object dictionaries expose both coordinate conventions",
            "`y0` is the bottom edge measured upward from the page bottom",
            "`y1` is the top edge measured upward from the page bottom",
            "`top` is the top edge measured downward from the page top",
            "`bottom` is the lower edge measured downward from the page top",
            "`doctop = initial_doctop + top`",
            "`initial_doctop` is the sum of displayed heights of preceding pages in the current page view",
            "Rust and WebAssembly object models expose displayed `BBox` geometry without Python `y0` and `y1` companions",
            "Do not compare `doctop` values from different document or selected-page views without retaining the view identity",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_boundary_names_sources_and_claim_limits_are_unambiguous(self) -> None:
        for field_name in (
            "bbox_display_top_left_points",
            "source_media_box_pdf_user_space",
            "top_page_points",
            "y0_page_bottom_points",
            "doctop_document_points",
        ):
            with self.subTest(field_name=field_name):
                self.assertIn(field_name, self.guide)

        for rule in (
            "Never infer coordinate space from a field named only `bbox`",
            "keep source page-box arrays separate from normalized display boxes",
            "validate finite values and ordered displayed boxes at the boundary",
            "coordinate-system documentation is not compatibility evidence",
            "does not approve a compatibility deviation",
        ):
            with self.subTest(rule=rule):
                self.assertIn(rule, self.compact_guide)

        for source in (
            "https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/page.py",
            "../crates/pdfplumber-core/src/geometry.rs",
            "../crates/pdfplumber-parse/src/page_geometry.rs",
            "../crates/pdfplumber/src/pdf.rs",
        ):
            with self.subTest(source=source):
                self.assertIn(source, self.guide)

        self.assertNotRegex(self.guide, re.compile(r"\b\d+(?:\.\d+)?%\b"))


if __name__ == "__main__":
    unittest.main()
