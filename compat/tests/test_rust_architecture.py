"""Contracts for the workspace and extraction architecture guide (DX-016)."""

from __future__ import annotations

import re
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GUIDE_PATH = ROOT / "docs/architecture.md"
README = (ROOT / "README.md").read_text(encoding="utf-8")
RUST_API = (ROOT / "docs/rust-api.md").read_text(encoding="utf-8")
CRATE_DOCS = (ROOT / "crates/pdfplumber/src/lib.rs").read_text(encoding="utf-8")
CHANGELOG = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
ROADMAP = (ROOT / "ROADMAP.md").read_text(encoding="utf-8")
PRD = (ROOT / "PRD.md").read_text(encoding="utf-8")
SUPPORT_SOURCE = (ROOT / "support-matrix.toml").read_text(encoding="utf-8")

WORKSPACE_CRATES = {
    "pdfplumber-core",
    "pdfplumber-parse",
    "pdfplumber",
    "pdfplumber-cli",
    "pdfplumber-py",
    "pdfplumber-wasm",
}

SOURCE_LINKS = {
    "../crates/pdfplumber/src/pdf.rs",
    "../crates/pdfplumber/src/page.rs",
    "../crates/pdfplumber-parse/src/backend.rs",
    "../crates/pdfplumber-parse/src/lopdf_backend.rs",
    "../crates/pdfplumber-parse/src/interpreter.rs",
    "../crates/pdfplumber-parse/src/handler.rs",
    "../crates/pdfplumber-core/src/words.rs",
    "../crates/pdfplumber-core/src/layout.rs",
    "../crates/pdfplumber-core/src/table.rs",
    "../crates/pdfplumber-cli/src/shared.rs",
    "../crates/pdfplumber-py/src/lib.rs",
    "../crates/pdfplumber-wasm/src/lib.rs",
}


class RustArchitectureContractTests(unittest.TestCase):
    def guide(self) -> str:
        self.assertTrue(GUIDE_PATH.is_file())
        if not GUIDE_PATH.is_file():
            return ""
        return GUIDE_PATH.read_text(encoding="utf-8")

    def test_all_six_workspace_crates_and_dependency_direction_are_mapped(self) -> None:
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
        members = {Path(member).name for member in manifest["workspace"]["members"]}
        self.assertEqual(members, WORKSPACE_CRATES)

        guide = self.guide()
        for crate in WORKSPACE_CRATES:
            with self.subTest(crate=crate):
                self.assertIn(f"`{crate}`", guide)
                self.assertIn(f"(../crates/{crate})", guide)

        normalized = " ".join(guide.split())
        self.assertRegex(normalized, r"pdfplumber-parse.*depends on.*pdfplumber-core")
        self.assertRegex(normalized, r"pdfplumber.*depends on.*pdfplumber-parse.*pdfplumber-core")
        self.assertRegex(normalized, r"pdfplumber-cli.*pdfplumber-py.*pdfplumber-wasm.*depend on.*facade")
        self.assertRegex(normalized, r"(?i)bindings.*do not feed back.*facade.*parser.*core")

        self.assertIn("docs/architecture.md", README)
        for crate in WORKSPACE_CRATES:
            self.assertIn(f"`{crate}`", README)

    def test_one_text_extraction_request_has_an_ordered_source_backed_flow(self) -> None:
        guide = self.guide()
        flow_start = guide.find("## One text extraction request")
        self.assertGreaterEqual(flow_start, 0)
        flow = guide[flow_start:]
        stages = (
            "`Pdf::open_bytes`",
            "`LopdfBackend::open`",
            "`Pdf::from_doc`",
            "`Pdf::page`",
            "`LopdfBackend::interpret_page`",
            "`interpret_content_stream`",
            "`ContentHandler`",
            "`Page::from_extraction`",
            "`Page::extract_text`",
            "`WordExtractor::extract`",
            "`words_to_text`",
        )
        positions = [flow.find(stage) for stage in stages]
        self.assertTrue(all(position >= 0 for position in positions), positions)
        self.assertEqual(positions, sorted(positions))

        for table_stage in ("`Page::find_tables`", "`derive_edges`", "`TableFinder::find_tables`"):
            self.assertIn(table_stage, flow)

        for link in SOURCE_LINKS:
            with self.subTest(link=link):
                self.assertIn(f"({link})", guide)
                self.assertTrue((GUIDE_PATH.parent / link).resolve().is_file())

    def test_cache_ownership_and_recomputation_are_explicit(self) -> None:
        normalized = " ".join(self.guide().split())

        for cache_name in (
            "page_ids",
            "page_widths",
            "page_heights",
            "metadata",
            "bookmarks",
            "structure_tree",
            "font_cache",
            "pages_cache",
            "objects_cache",
            "metadata_cache",
            "page_cache",
        ):
            with self.subTest(cache=cache_name):
                self.assertIn(f"`{cache_name}`", normalized)

        self.assertRegex(normalized, r"(?i)font_cache.*one.*interpretation")
        self.assertRegex(normalized, r"(?i)no.*Rust.*page-result cache")
        self.assertRegex(normalized, r"(?i)repeated.*Pdf::page.*reinterprets")
        self.assertRegex(normalized, r"(?i)AtomicUsize.*document-wide.*resource budgets")
        self.assertRegex(normalized, r"(?i)Python.*flush_cache.*close.*recompute")

    def test_stable_advanced_and_binding_extension_boundaries_are_distinct(self) -> None:
        guide = self.guide()
        normalized = " ".join(guide.split())

        for heading in (
            "### Stable application boundary",
            "### Advanced parser boundary",
            "### Binding boundary",
        ):
            self.assertIn(heading, guide)

        self.assertRegex(normalized, r"(?i)ordinary applications.*only.*pdfplumber.*facade")
        self.assertIn(
            "Implementing `PdfBackend` does not replace the backend inside `pdfplumber::Pdf`",
            guide,
        )
        self.assertRegex(normalized, r"(?i)ContentHandler.*low-level.*event")
        self.assertRegex(normalized, r"(?i)pdfplumber-core.*synthetic.*algorithm")
        self.assertRegex(normalized, r"(?i)binding adapters.*must not change.*extraction semantics")

    def test_contributor_change_map_names_each_ownership_layer(self) -> None:
        guide = self.guide()
        self.assertIn("## Where a change belongs", guide)
        for source_area in (
            "`crates/pdfplumber-parse/src/`",
            "`crates/pdfplumber-core/src/`",
            "`crates/pdfplumber/src/`",
            "`crates/pdfplumber-cli/src/`",
            "`crates/pdfplumber-py/src/`",
            "`crates/pdfplumber-wasm/src/`",
        ):
            with self.subTest(source_area=source_area):
                self.assertIn(source_area, guide)

        self.assertRegex(guide, r"(?i)PDF syntax.*font.*operator")
        self.assertRegex(guide, r"(?i)geometry.*word.*layout.*table")
        self.assertRegex(guide, r"(?i)opening.*page orchestration.*resource")
        self.assertRegex(guide, r"(?i)surface-specific.*conversion.*argument.*serialization")

    def test_primary_docs_support_changelog_roadmap_and_prd_are_traceable(self) -> None:
        for primary_document in (README, RUST_API, CRATE_DOCS):
            self.assertIn("architecture.md", primary_document)

        self.assertRegex(CHANGELOG, r"(?is)architecture guide.*six.*crate")
        self.assertIn("docs/architecture.md", SUPPORT_SOURCE)
        self.assertIn("compat/tests/test_rust_architecture.py", SUPPORT_SOURCE)
        self.assertNotIn("### Explain the extraction architecture", ROADMAP)
        self.assertIn("DX-017", ROADMAP)
        self.assertIn("- [x] **DX-016**", PRD)
        self.assertIn("- [x] **DOC-016**", PRD)
        self.assertRegex(
            PRD,
            r"(?m)^\| `DX-016` \| 2026-08-28 \| Codex \| PR #\d+ \|",
        )
        self.assertRegex(
            PRD,
            r"(?m)^\| `DOC-016` \| 2026-08-28 \| Codex \| PR #\d+ \|",
        )


if __name__ == "__main__":
    unittest.main()
