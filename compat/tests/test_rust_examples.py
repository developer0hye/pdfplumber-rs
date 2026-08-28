"""Compiled task-oriented Rust example contracts (DX-010)."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]
EXAMPLES = REPO_ROOT / "crates/pdfplumber/examples"

WORKFLOWS = {
    "extract_text": ("Pdf::open_path", "pdf.pages()", "TextOptions"),
    "extract_words": ("WordOptions", "extract_words", ".bbox"),
    "extract_table": ("TableSettings", "find_tables", ".rows"),
    "inspect_geometry": (".bbox()", ".lines()", ".rects()", ".curves()", ".images()"),
    "inspect_metadata": (".metadata()", ".title", ".author", "validate_metadata"),
    "open_encrypted": ("open_path_with_password", "password"),
    "handle_malformed": ("PdfErrorKind::Parse", ".kind()", "match Pdf::open_path"),
    "serialize_words": (
        "SERDE_JSON_SCHEMA",
        "serde_json::to_string_pretty",
        "WordOptions",
    ),
    "parallel_batch": ("pages_parallel()", "enumerate()", "Result"),
}


class RustExamplesContractTests(unittest.TestCase):
    def example_source(self, name: str) -> str:
        path = EXAMPLES / f"{name}.rs"
        self.assertTrue(
            path.is_file(), f"missing compiled example {path.relative_to(REPO_ROOT)}"
        )
        return path.read_text(encoding="utf-8")

    def test_examples_cover_every_required_workflow(self) -> None:
        for name, required_fragments in WORKFLOWS.items():
            with self.subTest(example=name):
                source = self.example_source(name)
                for fragment in required_fragments:
                    self.assertIn(fragment, source)

    def test_examples_are_complete_fallible_programs(self) -> None:
        for name in WORKFLOWS:
            with self.subTest(example=name):
                source = self.example_source(name)
                self.assertRegex(
                    source,
                    r"fn main\(\) -> Result<\(\), Box<dyn std::error::Error>>",
                )
                self.assertIn("std::env::args", source)
                self.assertIn("Usage:", source)
                self.assertIn("Ok(())", source)
                self.assertGreaterEqual(source.count("?"), 1)
                self.assertNotRegex(source, r"\.(?:unwrap|expect)\(")
                self.assertNotIn("std::process::exit", source)

    def test_feature_specific_examples_declare_their_build_contract(self) -> None:
        manifest = tomllib.loads(
            (REPO_ROOT / "crates/pdfplumber/Cargo.toml").read_text(encoding="utf-8")
        )
        examples = {entry["name"]: entry for entry in manifest.get("example", [])}

        self.assertEqual(examples["serialize_words"]["required-features"], ["serde"])
        self.assertEqual(examples["parallel_batch"]["required-features"], ["parallel"])

        workflow = (REPO_ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        command = "cargo check -p pdfplumber --examples --all-features"
        self.assertIn(command, workflow)
        self.assertNotRegex(
            workflow,
            rf"(?s)if: matrix\.rust == 'stable'.{{0,160}}{re.escape(command)}",
        )

    def test_public_guide_lists_copy_paste_commands_and_feature_boundaries(
        self,
    ) -> None:
        guide_path = REPO_ROOT / "docs/rust-examples.md"
        self.assertTrue(guide_path.is_file())
        guide = guide_path.read_text(encoding="utf-8")
        readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
        rust_api = (REPO_ROOT / "docs/rust-api.md").read_text(encoding="utf-8")

        for name in WORKFLOWS:
            with self.subTest(example=name):
                self.assertIn(f"cargo run -p pdfplumber --example {name}", guide)
        self.assertIn("--features serde", guide)
        self.assertIn("--features parallel", guide)
        self.assertIn("rust-examples.md", readme)
        self.assertIn("rust-examples.md", rust_api)

    def test_change_reference_and_roadmap_are_traceable(self) -> None:
        changelog = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        references = (REPO_ROOT / "references/INDEX.md").read_text(encoding="utf-8")
        reference_path = REPO_ROOT / "references/rust-examples.md"
        roadmap = (REPO_ROOT / "ROADMAP.md").read_text(encoding="utf-8")

        self.assertRegex(changelog, r"(?i)compiled.*Rust examples")
        self.assertTrue(reference_path.is_file())
        self.assertIn("rust-examples.md", references)
        self.assertIn("Cargo Targets", reference_path.read_text(encoding="utf-8"))
        self.assertIn("DX-017", roadmap)
        self.assertRegex(roadmap, r"(?i)API design")
        self.assertNotIn("DX-010", roadmap)


if __name__ == "__main__":
    unittest.main()
