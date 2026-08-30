"""Source-bound contracts for the parser and font limitations guide (DOC-014)."""

from __future__ import annotations

import hashlib
import re
import unittest
from pathlib import Path

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]
GUIDE_PATH = REPO_ROOT / "docs" / "parser-and-font-limitations.md"
PROVENANCE_PATH = REPO_ROOT / "compat" / "fixture-provenance.toml"
CHECKSUMS_PATH = REPO_ROOT / "tests" / "fixtures" / "checksums.sha256"


def compact(text: str) -> str:
    return " ".join(text.split())


def fixture_checksums() -> dict[str, str]:
    checksums: dict[str, str] = {}
    for line in CHECKSUMS_PATH.read_text(encoding="utf-8").splitlines():
        digest, relative_path = line.split(maxsplit=1)
        checksums[relative_path] = digest
    return checksums


class ParserFontLimitationsDocumentationContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.guide = (
            GUIDE_PATH.read_text(encoding="utf-8") if GUIDE_PATH.is_file() else ""
        )
        cls.compact_guide = compact(cls.guide)
        provenance = tomllib.loads(PROVENANCE_PATH.read_text(encoding="utf-8"))
        cls.external_fixtures = [
            fixture
            for fixture in provenance["fixtures"]
            if fixture["collection"] == "external-parser"
        ]

    def test_canonical_guide_is_linked_from_parser_entry_points(self) -> None:
        with self.subTest(artifact="canonical guide"):
            self.assertTrue(GUIDE_PATH.is_file(), f"missing guide: {GUIDE_PATH}")

        links = {
            "README.md": (
                "[parser and font limitations](docs/parser-and-font-limitations.md)"
            ),
            "crates/pdfplumber-cli/README.md": (
                "[parser and font limitations](../../docs/parser-and-font-limitations.md)"
            ),
            "crates/pdfplumber-py/README.md": (
                "[parser and font limitations](../../docs/parser-and-font-limitations.md)"
            ),
            "crates/pdfplumber-wasm/README.md": (
                "[parser and font limitations](../../docs/parser-and-font-limitations.md)"
            ),
            "docs/architecture.md": (
                "[parser and font limitations](parser-and-font-limitations.md)"
            ),
            "docs/errors-and-resource-limits.md": (
                "[parser and font limitations](parser-and-font-limitations.md)"
            ),
            "docs/faq.md": (
                "[parser and font limitations](parser-and-font-limitations.md)"
            ),
            "docs/pre-parity-python-migration.md": (
                "[parser and font limitations](parser-and-font-limitations.md)"
            ),
            "docs/python-migration.md": (
                "[parser and font limitations](parser-and-font-limitations.md)"
            ),
            "docs/rust-api.md": (
                "[parser and font limitations](parser-and-font-limitations.md)"
            ),
            "docs/rust-errors.md": (
                "[parser and font limitations](parser-and-font-limitations.md)"
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
                r"(?im)^- \*\*Parser and fonts:\*\* .*parser and font limitations",
            )

    def test_parser_recovery_and_warning_boundaries_are_source_bound(self) -> None:
        for statement in (
            "`lopdf` is the structural backend",
            "A successful `Pdf::open_*` call proves only that structural loading completed",
            "The backend may strip a leading preamble before the first structural load",
            "On load failure it can correct a bad `startxref` or restore a narrowly truncated terminal `startxref`/`%%EOF` suffix",
            "other structural failures remain fatal",
            "`tokenize_lenient` preserves operators parsed before and after a malformed token",
            "resumes one byte after the failed token start",
            "clears the partial operand stack",
            "A recovered page is not automatically compatible with pinned Python output",
            "warning collection is opt-in through `ExtractOptions::collect_warnings`",
            "`Page::warnings()` is currently a Rust-only diagnostic surface",
            "missing fonts and missing metrics can fall back and continue extraction",
            "filter and file-structure tasks remain unverified even when a fixture opens",
            "../crates/pdfplumber-parse/src/lopdf_backend.rs",
            "../crates/pdfplumber-parse/src/tokenizer.rs",
            "../crates/pdfplumber-parse/src/interpreter.rs",
            "../crates/pdfplumber/src/pdf.rs",
            "../crates/pdfplumber/src/page.rs",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

        backend_source = (
            REPO_ROOT / "crates/pdfplumber-parse/src/lopdf_backend.rs"
        ).read_text(encoding="utf-8")
        for source_contract in (
            "try_strip_preamble(bytes)",
            "try_repair_xref(bytes)",
            "lopdf::Document::load_mem(bytes)",
        ):
            with self.subTest(source_contract=source_contract):
                self.assertIn(source_contract, backend_source)

        tokenizer_source = (
            REPO_ROOT / "crates/pdfplumber-parse/src/tokenizer.rs"
        ).read_text(encoding="utf-8")
        for source_contract in (
            "pub fn tokenize_lenient",
            "pos = saved_pos + 1",
            "operand_stack.clear()",
        ):
            with self.subTest(source_contract=source_contract):
                self.assertIn(source_contract, tokenizer_source)

    def test_font_resolution_fallbacks_and_residuals_are_explicit(self) -> None:
        for statement in (
            "The emitted display name is separate from the normalized name used for metrics and encoding lookup",
            "Type 0 display names come from the descendant CIDFont descriptor",
            "subset prefixes are retained",
            "invalid UTF-8 PDF names retain Python bytes-`repr` spelling",
            "ToUnicode stream decoding or CMap parsing failure is currently collapsed to an absent map",
            "ToUnicode CMap → simple-font encoding → legacy CJK encoding → predefined Adobe collection map → Identity or `(cid:N)` fallback",
            "an explicit but incomplete ToUnicode CMap is authoritative",
            "an unmapped code from that map becomes `(cid:N)`",
            "the predefined Adobe-CNS1, Adobe-GB1, Adobe-Japan1, and Adobe-Korea1 tables are consulted only when ToUnicode is absent",
            "Identity-H or Identity-V does not by itself prove that every CID is a Unicode scalar",
            "missing CID metrics, missing simple-font metrics, and missing font resources use defaults after an optional warning",
            "fallback widths can change `adv`, bounding boxes, and word grouping",
            "font-name parity does not prove font-metric or parser parity",
            "../crates/pdfplumber-parse/src/cmap.rs",
            "../crates/pdfplumber-parse/src/cid_font.rs",
            "../crates/pdfplumber-parse/src/cjk_encoding.rs",
            "../crates/pdfplumber-parse/src/font_metrics.rs",
            "../crates/pdfplumber-parse/src/standard_fonts.rs",
            "../crates/pdfplumber-parse/src/type1.rs",
            "../crates/pdfplumber-parse/src/truetype.rs",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

        interpreter_source = (
            REPO_ROOT / "crates/pdfplumber-parse/src/interpreter.rs"
        ).read_text(encoding="utf-8")
        for source_contract in (
            "fn pdfminer_font_name",
            "fn extract_tounicode_cmap",
            "CMap::parse(&data).ok()",
            "if c.cmap.is_some()",
            'Some(format!("(cid:{})", rc.char_code))',
        ):
            with self.subTest(source_contract=source_contract):
                self.assertIn(source_contract, interpreter_source)

    def test_every_licensed_external_parser_fixture_is_referenced_by_digest(
        self,
    ) -> None:
        self.assertEqual(len(self.external_fixtures), 28)
        for fixture in self.external_fixtures:
            fixture_path = REPO_ROOT / fixture["path"]
            with self.subTest(fixture=fixture["path"], check="file"):
                self.assertTrue(fixture_path.is_file())
            with self.subTest(fixture=fixture["path"], check="digest"):
                self.assertEqual(
                    hashlib.sha256(fixture_path.read_bytes()).hexdigest(),
                    fixture["sha256"],
                )
            with self.subTest(fixture=fixture["path"], check="guide path"):
                self.assertIn(f"`{fixture['path']}`", self.guide)
            with self.subTest(fixture=fixture["path"], check="guide digest"):
                self.assertIn(f"`{fixture['sha256']}`", self.guide)

        checksums = fixture_checksums()
        focused_project_fixtures = (
            "generated/cjk_mixed.pdf",
            "generated/multi_font.pdf",
            "real-world/fonts-encoding/special-characters.pdf",
            "real-world/fonts-encoding/standard-14-fonts.pdf",
        )
        for relative_path in focused_project_fixtures:
            full_path = f"tests/fixtures/{relative_path}"
            with self.subTest(fixture=full_path):
                self.assertIn(f"`{full_path}`", self.guide)
                self.assertIn(f"`{checksums[relative_path]}`", self.guide)

        for statement in (
            "The fixture table is an inventory, not a support matrix",
            "Open only",
            "Exact text",
            "Bounded metric",
            "Known residual",
            "`cross_validation` uses ordered ratios and fixture-specific floors",
            "`accuracy_benchmark` uses nearest-neighbor F1 with a two-point coordinate tolerance",
            "neither threshold is exact compatibility evidence",
            "compat/fixture-provenance.toml",
            "crates/pdfplumber/tests/cross_validation.rs",
            "crates/pdfplumber/tests/accuracy_benchmark.rs",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_surface_and_claim_boundaries_remain_explicit(self) -> None:
        for statement in (
            "Rust | `Page::warnings()` when collection is enabled",
            "Python | no public parser-warning collection surface",
            "Command-Line Interface | no structured parser-warning output",
            "WebAssembly | no parser-warning output",
            "Do not infer support from a successful open, a non-panicking extraction, or a passing percentage threshold",
            "reproduce against pinned CPython 3.13, `pdfplumber==0.11.10`, and `pdfminer.six==20260107`",
            "DOC-014 changes no runtime behavior",
            "does not approve a compatibility deviation",
            "does not change the generated support matrix or readiness scorecard",
            "All `PARSE-*`, `FONT-*`, malformed-input, object, text, serialization, and strict section 10 tasks remain independently open",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

        self.assertIsNotNone(
            re.search(
                r"(?s)## How to read this guide.*"
                r"## Structural parser boundary.*"
                r"## Content-stream recovery and warnings.*"
                r"## Font and Unicode resolution.*"
                r"## Fixture evidence.*"
                r"## Surface matrix.*"
                r"## Troubleshooting workflow.*"
                r"## Validation and provenance.*"
                r"## Claim boundary",
                self.guide,
            )
        )
        self.assertNotRegex(
            self.compact_guide,
            re.compile(r"supports (?:all|every) (?:PDF|font|parser)", re.IGNORECASE),
        )

        prd = (REPO_ROOT / "PRD.md").read_text(encoding="utf-8")
        for task_prefix in ("PARSE-001", "PARSE-018", "FONT-001", "FONT-025"):
            with self.subTest(open_task=task_prefix):
                self.assertRegex(prd, rf"(?m)^- \[ \] \*\*{task_prefix}\*\*")


if __name__ == "__main__":
    unittest.main()
