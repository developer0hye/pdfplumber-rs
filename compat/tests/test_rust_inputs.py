from __future__ import annotations

import unittest
from pathlib import Path

from scripts import check_doc_quickstarts as quick_starts

ROOT = Path(__file__).resolve().parents[2]
README = ROOT / "README.md"
CRATE_ROOT = ROOT / "crates" / "pdfplumber" / "src" / "lib.rs"
PDF_SOURCE = ROOT / "crates" / "pdfplumber" / "src" / "pdf.rs"
CHANGELOG = ROOT / "CHANGELOG.md"
ROADMAP = ROOT / "ROADMAP.md"
LOPDF_REFERENCE = ROOT / "references" / "lopdf.md"


class RustInputApiContractTests(unittest.TestCase):
    def test_readme_presents_one_named_input_family(self) -> None:
        readme = README.read_text(encoding="utf-8")
        heading = "## Rust Input API"
        self.assertIn(heading, readme)
        if heading not in readme:
            return

        section = " ".join(quick_starts.section(readme, "Rust Input API").split())
        for constructor in (
            "`Pdf::open_path`",
            "`Pdf::open_bytes`",
            "`Pdf::open_reader`",
        ):
            with self.subTest(constructor=constructor):
                self.assertIn(constructor, section)
        self.assertIn("`std::io::Read`", section)
        self.assertIn("does not borrow", section)
        self.assertIn("current reader position through end-of-file", section)
        self.assertIn("`PdfErrorKind::Io`", section)
        self.assertIn("`PdfErrorKind::Parse`", section)

    def test_rustdoc_defines_input_ownership_and_error_contract(self) -> None:
        crate_docs = " ".join(CRATE_ROOT.read_text(encoding="utf-8").split())
        source = PDF_SOURCE.read_text(encoding="utf-8")

        self.assertIn("# Opening inputs", crate_docs)
        self.assertIn("[`Pdf::open_path`]", crate_docs)
        self.assertIn("[`Pdf::open_bytes`]", crate_docs)
        self.assertIn("[`Pdf::open_reader`]", crate_docs)

        for constructor in ("open_path", "open_bytes", "open_reader"):
            with self.subTest(constructor=constructor):
                self.assertRegex(source, rf"pub fn {constructor}\b")
        self.assertRegex(source, r"pub fn open_reader<R: std::io::Read>\(")
        self.assertIn("does not retain", source)
        self.assertIn("current position through end-of-file", source)
        self.assertIn("PdfErrorKind::Io", source)
        self.assertIn("PdfErrorKind::Parse", source)

    def test_password_inputs_follow_the_same_family_and_old_names_are_aliases(self) -> None:
        source = PDF_SOURCE.read_text(encoding="utf-8")

        for constructor in (
            "open_path_with_password",
            "open_bytes_with_password",
            "open_reader_with_password",
        ):
            with self.subTest(constructor=constructor):
                self.assertRegex(source, rf"pub fn {constructor}\b")

        self.assertIn("Self::open_path(path, options)", source)
        self.assertIn("Self::open_bytes(bytes, options)", source)
        self.assertIn("Self::open_bytes_with_password(bytes, password, options)", source)
        self.assertIn("Self::open_path_with_password(path, password, options)", source)

    def test_public_change_and_reference_are_traceable(self) -> None:
        changelog = CHANGELOG.read_text(encoding="utf-8")
        roadmap = ROADMAP.read_text(encoding="utf-8")
        reference = LOPDF_REFERENCE.read_text(encoding="utf-8")

        self.assertIn("`Pdf::open_path`", changelog)
        self.assertIn("`Pdf::open_bytes`", changelog)
        self.assertIn("`Pdf::open_reader`", changelog)
        self.assertIn("DX-018", roadmap)
        self.assertIn("8c454dd93d9c37e608c552a2b304d1d31d1cb2e1", reference)
        self.assertIn("Document::load_from<R: Read>", reference)
        self.assertIn("Document::load_mem", reference)


if __name__ == "__main__":
    unittest.main()
