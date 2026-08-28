"""Contracts for additive Cargo features and their verification matrix (DX-014)."""

from __future__ import annotations

import unittest
from pathlib import Path

import tomllib

ROOT = Path(__file__).resolve().parents[2]
WORKSPACE = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
FACADE_MANIFEST = tomllib.loads(
    (ROOT / "crates/pdfplumber/Cargo.toml").read_text(encoding="utf-8")
)
PARSER_MANIFEST = tomllib.loads(
    (ROOT / "crates/pdfplumber-parse/Cargo.toml").read_text(encoding="utf-8")
)
PYTHON_MANIFEST = tomllib.loads(
    (ROOT / "crates/pdfplumber-py/Cargo.toml").read_text(encoding="utf-8")
)
WASM_MANIFEST = tomllib.loads(
    (ROOT / "crates/pdfplumber-wasm/Cargo.toml").read_text(encoding="utf-8")
)
CI = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
CRATE_DOCS = (ROOT / "crates/pdfplumber/src/lib.rs").read_text(encoding="utf-8")
README = (ROOT / "README.md").read_text(encoding="utf-8")
RUST_API = (ROOT / "docs/rust-api.md").read_text(encoding="utf-8")
CHANGELOG = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
ROADMAP = (ROOT / "ROADMAP.md").read_text(encoding="utf-8")
PRD = (ROOT / "PRD.md").read_text(encoding="utf-8")


class RustFeatureContractTests(unittest.TestCase):
    def test_facade_features_are_explicit_additive_capabilities(self) -> None:
        self.assertEqual(WORKSPACE["workspace"]["resolver"], "3")
        features = FACADE_MANIFEST["features"]
        self.assertEqual(features["default"], ["std"])
        self.assertEqual(features["std"], [])
        self.assertEqual(features["serde"], ["pdfplumber-core/serde"])
        self.assertEqual(features["parallel"], ["dep:rayon"])
        self.assertEqual(set(features), {"default", "std", "serde", "parallel"})
        self.assertTrue(FACADE_MANIFEST["dependencies"]["rayon"]["optional"])

        source = "\n".join(
            path.read_text(encoding="utf-8")
            for path in (ROOT / "crates/pdfplumber/src").glob("*.rs")
        )
        self.assertNotRegex(source, r"cfg\s*!?\s*\([^\n]*not\s*\(\s*feature")
        self.assertNotIn("compile_error!", source)

    def test_workspace_integration_flags_are_audited(self) -> None:
        self.assertEqual(PARSER_MANIFEST["features"]["tracing"], ["dep:tracing"])
        self.assertTrue(PARSER_MANIFEST["dependencies"]["tracing"]["optional"])
        self.assertEqual(
            PYTHON_MANIFEST["features"]["extension-module"],
            ["pyo3/extension-module"],
        )
        wasm_facade = WASM_MANIFEST["dependencies"]["pdfplumber"]
        self.assertFalse(wasm_facade["default-features"])
        self.assertEqual(wasm_facade["features"], ["serde"])

    def test_ci_runs_representative_combinations_and_semantic_regressions(
        self,
    ) -> None:
        normalized = " ".join(CI.split())
        commands = (
            "cargo test -p pdfplumber --test feature_semantics --no-default-features",
            "cargo test -p pdfplumber --test feature_semantics",
            "cargo test -p pdfplumber --test feature_semantics --no-default-features --features serde",
            "cargo test -p pdfplumber --test feature_semantics --no-default-features --features parallel",
            "cargo test -p pdfplumber --test feature_semantics --all-features",
            "cargo test -p pdfplumber-core --features serde --test serde_roundtrip",
            "cargo test -p pdfplumber-parse --features tracing",
        )
        for command in commands:
            with self.subTest(command=command):
                self.assertIn(command, normalized)

        semantic_test = ROOT / "crates/pdfplumber/tests/feature_semantics.rs"
        self.assertTrue(semantic_test.is_file())
        source = semantic_test.read_text(encoding="utf-8")
        for fixture in ("basic_text.pdf", "table_lattice.pdf"):
            self.assertIn(fixture, source)
        for capability in (
            '#[cfg(feature = "std")]',
            '#[cfg(feature = "serde")]',
            '#[cfg(feature = "parallel")]',
        ):
            self.assertIn(capability, source)

    def test_public_guide_defines_defaults_boundaries_and_policy(self) -> None:
        guide_path = ROOT / "docs/rust-features.md"
        self.assertTrue(guide_path.is_file())
        guide = guide_path.read_text(encoding="utf-8")
        normalized = " ".join(guide.split())

        for feature in ("`std`", "`serde`", "`parallel`"):
            with self.subTest(feature=feature):
                self.assertIn(feature, guide)
        for entry in ("`Pdf::open_path`", "`Pdf::open_reader`", "`Pdf::open_bytes`"):
            self.assertIn(entry, guide)
        self.assertRegex(normalized, r"(?i)default.*std.*primary.*path")
        self.assertRegex(normalized, r"(?i)features.*additive")
        self.assertRegex(normalized, r"(?i)not.*no_std.*contract")
        self.assertRegex(normalized, r"(?i)runtime options.*extraction semantics")
        self.assertIn("pdfplumber-parse/tracing", guide)
        self.assertIn("pdfplumber-py/extension-module", guide)
        self.assertIn("default-features = false", guide)

    def test_primary_docs_sources_changelog_and_roadmap_are_traceable(self) -> None:
        for document in (README, RUST_API, CRATE_DOCS):
            self.assertIn("rust-features.md", document)
        self.assertRegex(CHANGELOG, r"(?is)feature.*matrix")

        reference = ROOT / "references/cargo-features.md"
        self.assertTrue(reference.is_file())
        reference_text = reference.read_text(encoding="utf-8")
        self.assertIn("Cargo Book", reference_text)
        self.assertRegex(reference_text, r"(?i)feature unification")
        self.assertRegex(reference_text, r"(?i)additive")
        self.assertIn("cargo-features.md", (ROOT / "references/INDEX.md").read_text())

        self.assertNotIn("### Make feature combinations predictable", ROADMAP)
        self.assertIn("DX-015", ROADMAP)
        self.assertIn("- [x] **DX-014**", PRD)
        self.assertRegex(
            PRD,
            r"(?m)^\| `DX-014` \| 2026-08-28 \| Codex \| PR #426 \|",
        )


if __name__ == "__main__":
    unittest.main()
