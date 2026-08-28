"""Contracts for the stable-facade API-design review criteria (DX-017)."""

from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GUIDE_PATH = ROOT / "docs/rust-api-design.md"
README = (ROOT / "README.md").read_text(encoding="utf-8")
RUST_API = (ROOT / "docs/rust-api.md").read_text(encoding="utf-8")
CRATE_DOCS = (ROOT / "crates/pdfplumber/src/lib.rs").read_text(encoding="utf-8")
CHANGELOG = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
ROADMAP = (ROOT / "ROADMAP.md").read_text(encoding="utf-8")
PRD = (ROOT / "PRD.md").read_text(encoding="utf-8")
REFERENCE_INDEX = (ROOT / "references/INDEX.md").read_text(encoding="utf-8")
SUPPORT_SOURCE = (ROOT / "support-matrix.toml").read_text(encoding="utf-8")


class RustApiDesignContractTests(unittest.TestCase):
    def guide(self) -> str:
        self.assertTrue(GUIDE_PATH.is_file())
        if not GUIDE_PATH.is_file():
            return ""
        return GUIDE_PATH.read_text(encoding="utf-8")

    def test_review_record_is_required_for_stable_facade_changes(self) -> None:
        guide = self.guide()
        normalized = " ".join(guide.split())

        self.assertRegex(normalized, r"(?i)stable.*pdfplumber.*facade")
        self.assertIn("## Required review record", guide)
        for field in (
            "Before and after signature",
            "Observable contract",
            "Ownership and allocation impact",
            "Compatibility classification",
            "Validation evidence",
        ):
            with self.subTest(field=field):
                self.assertIn(field, guide)
        self.assertRegex(normalized, r"(?i)not applicable.*reason")
        self.assertRegex(normalized, r"(?i)pull request.*before.*merge")

    def test_ownership_and_allocation_criteria_are_source_backed(self) -> None:
        guide = self.guide()
        normalized = " ".join(guide.split())

        for api in (
            "`Pdf::open_bytes`",
            "`Pdf::open_reader`",
            "`Pdf::pages`",
            "`Page`",
            "`Page::chars`",
            "`Page::edges`",
            "`Page::extract_text`",
        ):
            with self.subTest(api=api):
                self.assertIn(api, guide)
        self.assertRegex(normalized, r"(?i)caller.*decides.*clone")
        self.assertRegex(normalized, r"(?i)borrow.*does not outlive")
        self.assertRegex(normalized, r"(?i)allocation.*hot path.*measure")
        self.assertRegex(normalized, r"(?i)eager.*lazy.*materializ")
        self.assertIn("(../crates/pdfplumber/src/pdf.rs)", guide)
        self.assertIn("(../crates/pdfplumber/src/page.rs)", guide)

    def test_iterator_review_covers_laziness_fallibility_and_order(self) -> None:
        guide = self.guide()
        normalized = " ".join(guide.split())

        for concept in (
            "`Pages`",
            "`PagesIter`",
            "`Iterator<Item = Result<Page, PdfError>>`",
            "`DoubleEndedIterator`",
            "`ExactSizeIterator`",
            "`FusedIterator`",
            "`Pdf::pages_parallel`",
        ):
            with self.subTest(concept=concept):
                self.assertIn(concept, guide)
        self.assertRegex(normalized, r"(?i)lazy.*one page")
        self.assertRegex(normalized, r"(?i)page-index order")
        self.assertRegex(normalized, r"(?i)partial consumption.*drop")

    def test_determinism_criteria_reject_unspecified_map_iteration(self) -> None:
        guide = self.guide()
        normalized = " ".join(guide.split())

        self.assertIn("`HashMap`", guide)
        self.assertIn("`Page::chars_by_mcid`", guide)
        self.assertRegex(normalized, r"(?i)HashMap.*lookup.*not.*output order")
        self.assertRegex(normalized, r"(?i)tie-break")
        self.assertRegex(normalized, r"(?i)sequential.*parallel.*same.*order")
        self.assertRegex(normalized, r"(?i)repeat.*same input.*same output")
        self.assertRegex(normalized, r"(?i)serialized.*order")

    def test_error_composition_preserves_typed_context_and_one_source_chain(self) -> None:
        guide = self.guide()
        normalized = " ".join(guide.split())

        for api in (
            "`PdfError`",
            "`PdfErrorKind`",
            "`PdfErrorContext`",
            "`std::error::Error::source`",
            "`Result`",
        ):
            with self.subTest(api=api):
                self.assertIn(api, guide)
        self.assertRegex(normalized, r"(?i)anticipated.*failure.*Result")
        self.assertRegex(normalized, r"(?i)source.*exactly once")
        self.assertRegex(normalized, r"(?i)Display.*source.*not both")
        self.assertRegex(normalized, r"(?i)iterator.*error.*continue|continue.*iterator.*error")
        self.assertIn("(rust-errors.md)", guide)

    def test_extension_trait_criteria_keep_advanced_hooks_outside_the_facade(self) -> None:
        guide = self.guide()
        normalized = " ".join(guide.split())

        self.assertIn("## Extension traits", guide)
        self.assertIn("`PdfBackend`", guide)
        self.assertIn("`ContentHandler`", guide)
        self.assertRegex(normalized, r"(?i)no.*stable facade extension trait")
        self.assertRegex(normalized, r"(?i)inherent method.*first")
        self.assertRegex(normalized, r"(?i)downstream implement")
        self.assertRegex(normalized, r"(?i)seal")
        self.assertRegex(normalized, r"(?i)method.*collision")
        self.assertRegex(normalized, r"(?i)dyn compatibility|object safe")

    def test_future_compatibility_accounts_for_fields_enums_traits_and_arity(self) -> None:
        guide = self.guide()
        normalized = " ".join(guide.split())

        for concept in (
            "`#[non_exhaustive]`",
            "`PdfErrorKind`",
            "`ExtractOptions`",
            "`TextOptions`",
            "`WordOptions`",
            "`TableSettings`",
            "`cargo-semver-checks`",
        ):
            with self.subTest(concept=concept):
                self.assertIn(concept, guide)
        self.assertRegex(normalized, r"(?i)public field.*compatibility commitment")
        self.assertRegex(normalized, r"(?i)adding.*argument.*breaking")
        self.assertRegex(normalized, r"(?i)trait.*non-defaulted.*breaking")
        self.assertRegex(normalized, r"(?i)behavior.*document.*contract")

    def test_primary_docs_sources_support_roadmap_and_prd_are_traceable(self) -> None:
        for primary_document in (README, RUST_API, CRATE_DOCS):
            self.assertIn("rust-api-design.md", primary_document)

        self.assertRegex(CHANGELOG, r"(?is)API-design review.*ownership.*determinism")
        self.assertIn("rust-api-design.md", REFERENCE_INDEX)
        self.assertIn("docs/rust-api-design.md", SUPPORT_SOURCE)
        self.assertIn("compat/tests/test_rust_api_design.py", SUPPORT_SOURCE)
        self.assertNotIn("### Review Rust API design", ROADMAP)
        self.assertIn("SCORE-010", ROADMAP)
        self.assertIn("- [x] **DX-017**", PRD)
        self.assertRegex(
            PRD,
            r"(?m)^\| `DX-017` \| 2026-08-28 \| Codex \| PR #\d+ \|",
        )


if __name__ == "__main__":
    unittest.main()
