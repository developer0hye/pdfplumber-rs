"""Contracts for the complete visual-debugging guide (DOC-011)."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GUIDE_PATH = REPO_ROOT / "docs" / "visual-debugging.md"


def compact(text: str) -> str:
    return " ".join(text.split())


class VisualDebuggingDocumentationContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.guide = (
            GUIDE_PATH.read_text(encoding="utf-8") if GUIDE_PATH.is_file() else ""
        )
        cls.compact_guide = compact(cls.guide)

    def test_canonical_guide_is_linked_from_every_debugging_entry_point(self) -> None:
        with self.subTest(artifact="canonical guide"):
            self.assertTrue(GUIDE_PATH.is_file(), f"missing guide: {GUIDE_PATH}")

        links = {
            "README.md": "[visual-debugging guide](docs/visual-debugging.md)",
            "crates/pdfplumber-py/README.md": (
                "[visual-debugging guide](../../docs/visual-debugging.md)"
            ),
            "crates/pdfplumber-wasm/README.md": (
                "[visual-debugging guide](../../docs/visual-debugging.md)"
            ),
            "docs/faq.md": "[visual-debugging guide](visual-debugging.md)",
            "docs/pre-parity-python-migration.md": (
                "[visual-debugging guide](visual-debugging.md)"
            ),
            "docs/python-migration.md": (
                "[visual-debugging guide](visual-debugging.md)"
            ),
            "docs/rust-api.md": "[visual-debugging guide](visual-debugging.md)",
            "docs/table-settings.md": (
                "[visual-debugging guide](visual-debugging.md)"
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
                r"(?im)^- \*\*Visual debugging:\*\* .*visual-debugging guide",
            )

    def test_pinned_python_dependencies_and_render_options_are_exact(self) -> None:
        for statement in (
            "`pdfplumber==0.11.10` requires `Pillow>=12.2.0` and `pypdfium2>=5.9.0`",
            "the pinned environment resolved Pillow 12.3.0 and pypdfium2 5.13.0",
            "`Wand`, ImageMagick, and Ghostscript are not dependencies of ordinary `.to_image()`",
            "Ghostscript is used only by the separate `repair=True` path",
            "opens the same path or rewinds the in-memory stream to byte zero",
            "forwards the PDF password to `pypdfium2.PdfDocument`",
            "selects `page_number - 1`",
            "renders at `scale=resolution / 72`",
            "sets `prefer_bgrx=True`",
            "disables text, path, and image smoothing unless `antialias=True`",
            "converts the rendered bitmap to `RGB`",
            "only one of `resolution`, `width`, and `height` may be supplied",
            "`resolution` defaults to 72 pixels per inch",
            "`width` computes `resolution = 72 * width / page.width`",
            "`height` computes `resolution = 72 * height / page.height`",
            "`resolution=0` falls back to 72",
            "`antialias=False`",
            "`force_mediabox=False`",
            "`width=503` produces `(503, 306)`",
            "`height=805` produces `(1326, 805)`",
            "`resolution=150` produces `(2100, 1275)`",
            "Only one of these arguments can be provided: resolution, width, height. You provided 2",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_pageimage_state_drawing_and_output_behavior_is_complete(self) -> None:
        for method in (
            "`reset()`",
            "`copy()`",
            "`show()`",
            "`save()`",
            "`draw_line()`",
            "`draw_lines()`",
            "`draw_vline()`",
            "`draw_vlines()`",
            "`draw_hline()`",
            "`draw_hlines()`",
            "`draw_rect()`",
            "`draw_rects()`",
            "`draw_circle()`",
            "`draw_circles()`",
            "`outline_words()`",
            "`outline_chars()`",
            "`debug_table()`",
            "`debug_tablefinder()`",
            "`_repr_png_()`",
        ):
            with self.subTest(method=method):
                self.assertIn(method, self.guide)

        for statement in (
            "`original` is the unannotated RGB bitmap",
            "`annotated` is a separate RGB working image",
            "drawing methods mutate `annotated` and return the same `PageImage`",
            "page coordinates are translated by the displayed bounding box, multiplied by `scale`, and truncated with `int`",
            "raw point sequences, object dictionaries, and pandas collections",
            "line objects prefer `pts` and otherwise use `x0`, `top`, `x1`, and `bottom`",
            "rectangle objects use `x0`, `top`, `x1`, and `bottom`",
            "circle objects use the center of that object box",
            "the default fill is blue with alpha 50",
            "the default stroke is red with alpha 200",
            "the default stroke width is one image pixel",
            "`reset()` discards every overlay by rebuilding `annotated` from `original`",
            "`copy()` shares the same original image object but starts with a clean overlay",
            "`copy()` does not preserve a non-default resolution value",
            "`show()` delegates to Pillow and may launch an external local viewer",
            "Jupyter calls `_repr_png_()` and receives PNG bytes",
            "`save()` writes `annotated`, not `original`",
            "PNG, `quantize=True`, 256 colors, and 8 bits",
            "quantization uses Pillow's fast octree method and palette mode",
            "the saved DPI is `(resolution, resolution)`",
            "`quantize=False` keeps the RGB annotated image",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_crop_filter_table_and_error_boundaries_are_unambiguous(self) -> None:
        for statement in (
            "cropped pages render the root PDF page and then crop the bitmap",
            "a crop box `(10, 20, 30, 50)` produces a `(20, 30)` image at 72 pixels per inch",
            "`force_mediabox=True` affects an original page whose media box differs from its crop box",
            "it does not override an explicit `CroppedPage` bounding box",
            "filtered pages cannot remove or alter content in the underlying raster",
            "their filtered objects can still be drawn as overlays",
            "`Page.debug_tablefinder()` returns data and does not draw",
            "`PageImage.debug_tablefinder()` draws and returns the image",
            "a `TableFinder`, a `TableSettings`, a dictionary, or `None`",
            "tables are drawn first, then edges, then intersections",
            "table cells use the default blue fill and red stroke",
            "intersections are transparent circles with a blue stroke and radius 3",
            "Argument must be instance of TableFinderor a TableFinder settings dict.",
            "MalformedPDFException: Failed to load document (PDFium: Data format error).",
            "every call to `.to_image()` creates a new `PageImage`",
            "image generation does not mutate the page object cache",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

        self.assertGreaterEqual(self.guide.count("```python"), 6)

    def test_current_surfaces_sources_and_claim_boundary_remain_separate(self) -> None:
        for statement in (
            "Pinned Python `pdfplumber` produces a raster page image with raster overlays",
            "Rust produces SVG markup on a white page-coordinate canvas",
            "the Rust SVG contains no rendered PDF background",
            "`Page::to_svg` adds only the page boundary unless the caller uses `SvgRenderer` drawing methods",
            "`SvgOptions` controls output width, height, and scale without changing the view box",
            "`SvgDebugOptions` independently controls edges, intersections, cells, and tables",
            "all four SVG debug flags default to `true`",
            "the Rust SVG path requires no Pillow, PDFium, ImageMagick, Wand, or Ghostscript runtime",
            "the `pdfplumber debug` Command-Line Interface writes SVG",
            "ordinary debug mode draws chars, lines, rects, edges, and table cells",
            "`--tables` draws table-pipeline edges, intersections, cells, and tables",
            "multiple selected pages append `_pageN` before the requested extension",
            "the extension does not change the SVG content format",
            "the current Python adapter does not expose `Page.to_image`, `PageImage`, or visual-debug drawing methods",
            "the current WebAssembly adapter exposes no raster or SVG visual-debugging method",
            "does not establish visual-debugging compatibility",
            "visual-debugging documentation is not compatibility evidence",
            "does not approve a compatibility deviation",
            "DOC-011 changes no runtime behavior",
            "https://github.com/jsvine/pdfplumber/blob/v0.11.10/README.md",
            "https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/display.py",
            "https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/page.py",
            "4b915da24aa7cc7066bdec0e8aebc0457fd1783c",
            "286e7e158c12da8305520ecc1f550f3bd8f1a906",
            "f6f1ce3e0e546b854787aff946601af44fcc6f69",
            "../crates/pdfplumber-core/src/svg.rs",
            "../crates/pdfplumber/src/page.rs",
            "../crates/pdfplumber-cli/src/debug_cmd.rs",
            "../crates/pdfplumber-py/python/pdfplumber/_native.pyi",
            "../crates/pdfplumber-wasm/src/lib.rs",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

        self.assertIsNotNone(
            re.search(
                r"(?s)## Current project surfaces.*"
                r"## Validation and provenance.*"
                r"## Claim boundary",
                self.guide,
            )
        )
        self.assertNotRegex(self.guide, re.compile(r"\b\d+(?:\.\d+)?%\b"))


if __name__ == "__main__":
    unittest.main()
