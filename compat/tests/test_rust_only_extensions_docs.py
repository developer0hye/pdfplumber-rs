"""Source-bound contracts for the Rust-native extensions guide (DOC-015)."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GUIDE_PATH = REPO_ROOT / "docs" / "rust-extensions.md"


def compact(text: str) -> str:
    return " ".join(text.split())


class RustOnlyExtensionsDocumentationContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.guide = (
            GUIDE_PATH.read_text(encoding="utf-8") if GUIDE_PATH.is_file() else ""
        )
        cls.compact_guide = compact(cls.guide)

    def test_canonical_guide_is_linked_from_extension_entry_points(self) -> None:
        with self.subTest(artifact="canonical guide"):
            self.assertTrue(GUIDE_PATH.is_file(), f"missing guide: {GUIDE_PATH}")

        links = {
            "README.md": "[Rust-native extensions](docs/rust-extensions.md)",
            "crates/pdfplumber-cli/README.md": (
                "[Rust-native extensions](../../docs/rust-extensions.md)"
            ),
            "crates/pdfplumber-py/README.md": (
                "[Rust-native extensions](../../docs/rust-extensions.md)"
            ),
            "crates/pdfplumber-wasm/README.md": (
                "[Rust-native extensions](../../docs/rust-extensions.md)"
            ),
            "docs/architecture.md": "[Rust-native extensions](rust-extensions.md)",
            "docs/page-numbering.md": "[Rust-native extensions](rust-extensions.md)",
            "docs/pre-parity-python-migration.md": (
                "[Rust-native extensions](rust-extensions.md)"
            ),
            "docs/python-migration.md": "[Rust-native extensions](rust-extensions.md)",
            "docs/rust-api.md": "[Rust-native extensions](rust-extensions.md)",
            "docs/rust-features.md": "[Rust-native extensions](rust-extensions.md)",
            "docs/table-settings.md": "[Rust-native extensions](rust-extensions.md)",
            "docs/visual-debugging.md": "[Rust-native extensions](rust-extensions.md)",
        }
        for relative, link in links.items():
            with self.subTest(document=relative):
                rendered = (REPO_ROOT / relative).read_text(encoding="utf-8")
                self.assertIn(link, rendered)

        changelog = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        with self.subTest(document="CHANGELOG.md"):
            self.assertRegex(
                changelog,
                r"(?im)^- \*\*Rust extensions:\*\* .*Rust-native extensions",
            )

    def test_every_extension_task_and_current_surface_is_inventoryed(self) -> None:
        prd = (REPO_ROOT / "PRD.md").read_text(encoding="utf-8")
        for number in range(1, 16):
            task = f"EXT-{number:03d}"
            with self.subTest(task=task, check="open PRD task"):
                self.assertRegex(prd, rf"(?m)^- \[ \] \*\*{task}\*\*")
            with self.subTest(task=task, check="guide row"):
                self.assertRegex(self.guide, rf"(?m)^\| `{task}` \|")

        source_contracts = {
            "crates/pdfplumber/src/page.rs": (
                "pub fn export_images(",
                "pub fn semantic_chars(",
                "pub fn extract_text_body(",
                "pub fn to_html(",
                "pub fn to_svg(",
                "pub fn debug_tablefinder_svg(",
            ),
            "crates/pdfplumber/src/pdf.rs": (
                "pub fn bookmarks(",
                "pub fn form_fields(",
                "pub fn extract_images_with_content(",
                "pub fn pages_parallel(",
                "pub fn validate(",
                "pub fn signatures(",
                "pub fn detect_page_regions(",
            ),
            "crates/pdfplumber-core/src/table.rs": (
                "pub min_accuracy: Option<f64>",
                "pub duplicate_merged_content: bool",
                "pub fn accuracy(&self) -> f64",
            ),
            "crates/pdfplumber-py/src/lib.rs": (
                '#[pyclass(name = "RustPDF", module = "pdfplumber._native")]',
                "fn extract_images(&self, py: Python<'_>, page_index: usize)",
                "fn accuracy(&self) -> f64",
            ),
            "crates/pdfplumber-wasm/src/lib.rs": (
                "pub struct WasmPdf",
                "pub struct WasmPage",
                "pub fn extract_text(&self, layout: Option<bool>)",
            ),
            "crates/pdfplumber-cli/src/cli.rs": (
                "Bookmarks",
                "Forms",
                "Images",
                "Validate",
                "Debug",
            ),
        }
        for relative, contracts in source_contracts.items():
            source = (REPO_ROOT / relative).read_text(encoding="utf-8")
            for contract in contracts:
                with self.subTest(source=relative, contract=contract):
                    self.assertIn(contract, source)

        for statement in (
            "Rust | Python `document.rust` | Command-Line Interface | WebAssembly",
            "Image extraction and export",
            "Bookmarks and outlines",
            "AcroForm fields",
            "Digital-signature inspection",
            "HTML export",
            "SVG and table-debug SVG",
            "Semantic and structure traversal",
            "Validation and extraction warnings",
            "Table quality and merged-content normalization",
            "Multi-column and page-region reading order",
            "Parallel page processing",
            "WebAssembly bindings",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_python_namespace_and_schema_boundary_are_explicit(self) -> None:
        for method in (
            "`document.rust.bookmarks()`",
            "`document.rust.form_fields()`",
            "`document.rust.signatures()`",
            "`document.rust.validate()`",
            "`document.rust.extract_images(page_index)`",
        ):
            with self.subTest(method=method):
                self.assertIn(method, self.compact_guide)

        for statement in (
            "pinned `pdfplumber==0.11.10` exposes none of these five methods on `PDF`",
            "`RustPDF` is implemented by the private `pdfplumber._native` module",
            "page indexes, bookmark destinations, and form-field page indexes remain zero-based",
            "every call returns native-shape dictionaries rather than compatibility dictionaries",
            "compatibility `PDF`, `Page`, `PDF.objects`, `to_dict`, `to_json`, and `to_csv` do not call this namespace",
            "signature inspection reports field metadata and `is_signed`; it does not perform cryptographic verification",
            "image results contain an `image` dictionary, raw `data` bytes, `format`, `width`, and `height`",
            "`Table.accuracy` remains directly exposed on the current Python `Table` class rather than under `document.rust`",
            "this unnamespaced exception is a collision risk, not a completed namespacing contract",
            "../crates/pdfplumber-py/src/lib.rs",
            "../crates/pdfplumber-py/python/pdfplumber/_native.pyi",
            "../crates/pdfplumber-py/tests/test_native_layout.py",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_observable_limits_for_each_family_are_source_bound(self) -> None:
        for statement in (
            "`Page::export_images` skips images without populated `Image.data` and therefore requires `ExtractOptions::extract_image_data`",
            "`Pdf::extract_images_with_content` decodes named image XObjects on demand and skips an image whose bytes cannot be extracted",
            "bookmarks are a flattened outline with `level` retaining nesting depth",
            "form and signature readers return an empty collection when the relevant AcroForm data is absent",
            "signature dates are retained as PDF strings rather than validated timestamps",
            "`Pdf::validate` is a bounded native structural checker, not a full ISO conformance certificate",
            "`Page::warnings()` is available only when warning collection was enabled before extraction",
            "HTML headings, emphasis, lists, blocks, columns, and table placement are heuristic",
            "`Page::to_svg` draws only the page boundary with default options",
            "structure traversal requires tagged content and MCID associations",
            "`min_accuracy = None` disables filtering and `duplicate_merged_content = false` leaves merged-cell placeholders unchanged",
            "page-region detection masks digit runs, requires a minimum page count, and scans configurable header and footer margins",
            "parallel results retain page-index order, but shared resource budgets can make the first reported limit failure scheduling-dependent",
            "WebAssembly exposes bytes-only open, metadata, page access, characters, text, words, tables, and search",
            "WebAssembly does not expose the document inspection, image-byte export, HTML, SVG, validation, warning, structure, or parallel APIs listed above",
            "../crates/pdfplumber/src/page.rs",
            "../crates/pdfplumber/src/pdf.rs",
            "../crates/pdfplumber-core/src/images.rs",
            "../crates/pdfplumber-core/src/html.rs",
            "../crates/pdfplumber-core/src/page_regions.rs",
            "../crates/pdfplumber-core/src/table.rs",
            "../crates/pdfplumber-core/src/validation.rs",
            "../crates/pdfplumber-core/src/signature.rs",
            "../crates/pdfplumber-wasm/src/lib.rs",
            "../crates/pdfplumber-cli/src/cli.rs",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_feature_maturity_and_compatibility_claims_remain_separate(self) -> None:
        for statement in (
            "A Cargo feature is a compile-time capability switch, not an enhanced compatibility mode",
            "`std` adds path constructors, `serde` adds trait implementations, and `parallel` adds `Pdf::pages_parallel`",
            "enabling a feature must not change sequential extraction values or strict Python output",
            "extension output is neither parity evidence nor an approved deviation",
            "Rust release `0.3.0` is alpha",
            "the Python distribution is alpha",
            "the Command-Line Interface is alpha",
            "the WebAssembly package is experimental",
            "DOC-015 changes documentation only",
            "does not stabilize any extension API",
            "does not change runtime behavior, fixtures, thresholds, tolerances, generated support/readiness artifacts, or compatibility results",
            "`PDF-035`, `EXT-001` through `EXT-015`, and strict section 10 remain open",
            "No extension family is promised on every surface",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

        self.assertIsNotNone(
            re.search(
                r"(?s)## How to read this guide.*"
                r"## Surface and maturity matrix.*"
                r"## Document inspection and assets.*"
                r"## Rendering and semantic traversal.*"
                r"## Tables, layout, and concurrency.*"
                r"## Python `document\.rust` namespace.*"
                r"## Command-Line Interface and WebAssembly boundaries.*"
                r"## Compatibility implications.*"
                r"## Validation and source map.*"
                r"## Claim boundary",
                self.guide,
            )
        )
        self.assertNotRegex(
            self.compact_guide,
            re.compile(r"(?:fully|completely) (?:stable|compatible)", re.IGNORECASE),
        )


if __name__ == "__main__":
    unittest.main()
