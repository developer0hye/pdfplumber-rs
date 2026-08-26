from __future__ import annotations

import inspect
import unittest
from pathlib import Path

from scripts import check_doc_quickstarts as quick_starts

ROOT = Path(__file__).resolve().parents[2]
README = ROOT / "README.md"
CRATE_ROOT = ROOT / "crates" / "pdfplumber" / "src" / "lib.rs"
WORKFLOW = ROOT / ".github" / "workflows" / "ci.yml"


class RustFacadeContractTests(unittest.TestCase):
    def test_readme_defines_pdf_as_the_only_ordinary_entry_path(self) -> None:
        readme = README.read_text(encoding="utf-8")
        heading = "## Rust API Boundary"
        self.assertIn(heading, readme)
        if heading not in readme:
            return

        boundary = " ".join(quick_starts.section(readme, "Rust API Boundary").split())
        self.assertIn("`Pdf` is the canonical high-level entry point", boundary)
        self.assertIn(
            "do not add direct dependencies on `pdfplumber-core` or "
            "`pdfplumber-parse`",
            boundary,
        )
        self.assertIn("(compat/tests/test_rust_facade.py)", boundary)

    def test_crate_root_does_not_reexport_parser_internal_types(self) -> None:
        source = CRATE_ROOT.read_text(encoding="utf-8")

        self.assertIn("pub use pdf::{Pages, PagesIter, Pdf};", source)
        self.assertIn("PdfError", source)
        self.assertNotIn("pub use pdfplumber_parse::", source)
        for parser_internal in (
            "CharEvent",
            "ContentHandler",
            "ImageEvent",
            "LopdfBackend",
            "LopdfDocument",
            "LopdfPage",
            "PageGeometry",
            "PaintOp",
            "PathEvent",
            "PdfBackend",
        ):
            with self.subTest(parser_internal=parser_internal):
                self.assertNotRegex(
                    source,
                    rf"(?m)^pub use .*\b{parser_internal}\b",
                )

    def test_rustdoc_names_the_same_high_level_boundary(self) -> None:
        source = " ".join(CRATE_ROOT.read_text(encoding="utf-8").split())

        self.assertIn("ordinary applications should depend only on this crate", source)
        self.assertIn("[`Pdf`] is the canonical high-level entry point", source)
        self.assertIn("Parser-internal types are intentionally not re-exported", source)

    def test_clean_consumer_compiles_only_the_pdfplumber_facade_in_ci(self) -> None:
        installation, snippets = quick_starts.surface_snippets("rust")
        self.assertEqual(installation, '[dependencies]\npdfplumber = "0.3"')
        self.assertTrue(snippets)
        self.assertIn("use pdfplumber::{Pdf, TextOptions};", snippets[0])

        runner = inspect.getsource(quick_starts.run_rust_quick_starts)
        self.assertIn("SURFACES['rust'].installation", runner)
        self.assertNotIn("pdfplumber-core", runner)
        self.assertNotIn("pdfplumber-parse", runner)

        workflow = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn(
            "python scripts/check_doc_quickstarts.py --rust --cli",
            workflow,
        )


if __name__ == "__main__":
    unittest.main()
