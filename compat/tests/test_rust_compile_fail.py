"""Compile-fail rustdoc contracts for confusing facade misuse (DX-011)."""

from __future__ import annotations

import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
LIB_RS = (REPO_ROOT / "crates/pdfplumber/src/lib.rs").read_text(encoding="utf-8")
PDF_RS = (REPO_ROOT / "crates/pdfplumber/src/pdf.rs").read_text(encoding="utf-8")


class RustCompileFailContractTests(unittest.TestCase):
    def test_three_confusing_misuses_are_compile_fail_doctests(self) -> None:
        combined = f"{LIB_RS}\n{PDF_RS}"
        self.assertEqual(combined.count("```compile_fail"), 3)

        for fragment in (
            "fn map_pages_directly",
            "pdf.pages().map(",
            "fn dangling_pages",
            "Result<Pages<'_>, PdfError>",
            "PdfError::ParseError",
        ):
            with self.subTest(fragment=fragment):
                self.assertIn(fragment, combined)

    def test_each_failure_has_a_compiling_alternative_and_explanation(self) -> None:
        combined = f"{LIB_RS}\n{PDF_RS}"
        for fragment in (
            "`IntoIterator` rather than `Iterator`",
            "cannot outlive",
            "opaque `PdfError` has no public variants",
            "fn collect_page_numbers",
            "fn first_page",
            "fn is_parse_error",
        ):
            with self.subTest(fragment=fragment):
                self.assertIn(fragment, combined)

        self.assertGreaterEqual(combined.count("```no_run"), 6)

    def test_ci_runs_all_feature_doctests_on_current_stable(self) -> None:
        workflow = (REPO_ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        command = "cargo test -p pdfplumber --doc --all-features"
        self.assertIn(command, workflow)
        self.assertIn("dtolnay/rust-toolchain@stable", workflow)
        self.assertNotIn("matrix.rust", workflow)

    def test_public_docs_explain_the_intent_and_link_the_doctests(self) -> None:
        rust_api = (REPO_ROOT / "docs/rust-api.md").read_text(encoding="utf-8")
        readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
        normalized = " ".join(rust_api.split())

        self.assertRegex(normalized, r"(?i)compile-fail.*Pages.*Iterator")
        self.assertRegex(normalized, r"(?i)Pages.*outlive.*Pdf")
        self.assertRegex(normalized, r"(?i)PdfError.*kind")
        self.assertIn("compile-fail", readme)
        self.assertIn("test_rust_compile_fail.py", readme)

    def test_change_reference_and_roadmap_are_traceable(self) -> None:
        changelog = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        references = (REPO_ROOT / "references/INDEX.md").read_text(encoding="utf-8")
        reference_path = REPO_ROOT / "references/rust-compile-fail.md"
        roadmap = (REPO_ROOT / "ROADMAP.md").read_text(encoding="utf-8")

        self.assertRegex(changelog, r"(?i)compile-fail.*documentation tests")
        self.assertTrue(reference_path.is_file())
        self.assertIn("rust-compile-fail.md", references)
        self.assertIn("`compile_fail`", reference_path.read_text(encoding="utf-8"))
        self.assertIn("DX-016", roadmap)
        self.assertRegex(roadmap, r"(?i)extraction architecture")
        self.assertNotIn("DX-011", roadmap)


if __name__ == "__main__":
    unittest.main()
