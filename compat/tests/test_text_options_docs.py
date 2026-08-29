"""Contracts for the complete text-option guide (DOC-008)."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GUIDE_PATH = REPO_ROOT / "docs" / "text-options.md"

WORD_OPTIONS = {
    "x_tolerance": "3",
    "y_tolerance": "3",
    "x_tolerance_ratio": "None",
    "y_tolerance_ratio": "None",
    "keep_blank_chars": "False",
    "use_text_flow": "False",
    "vertical_ttb": "True",
    "horizontal_ltr": "True",
    "line_dir": '"ttb"',
    "char_dir": '"ltr"',
    "line_dir_rotated": "None",
    "char_dir_rotated": "None",
    "extra_attrs": "None",
    "split_at_punctuation": "False",
    "expand_ligatures": "True",
}

TEXTMAP_OPTIONS = {
    "layout": "False",
    "layout_width": "0",
    "layout_height": "0",
    "layout_width_chars": "0",
    "layout_height_chars": "0",
    "layout_bbox": "(0, 0, 0, 0)",
    "x_density": "7.25",
    "y_density": "13",
    "x_shift": "0",
    "y_shift": "0",
    "char_dir_render": "None",
    "line_dir_render": "None",
    "presorted": "False",
}


def compact(text: str) -> str:
    return " ".join(text.split())


class TextOptionsDocumentationContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.guide = (
            GUIDE_PATH.read_text(encoding="utf-8") if GUIDE_PATH.is_file() else ""
        )
        cls.compact_guide = compact(cls.guide)

    def test_canonical_guide_is_linked_from_every_text_entry_point(self) -> None:
        with self.subTest(artifact="canonical guide"):
            self.assertTrue(GUIDE_PATH.is_file(), f"missing guide: {GUIDE_PATH}")

        links = {
            "README.md": "[text-option guide](docs/text-options.md)",
            "crates/pdfplumber-py/README.md": (
                "[text-option guide](../../docs/text-options.md)"
            ),
            "crates/pdfplumber-wasm/README.md": (
                "[text-option guide](../../docs/text-options.md)"
            ),
            "docs/faq.md": "[text-option guide](text-options.md)",
            "docs/pre-parity-python-migration.md": (
                "[text-option guide](text-options.md)"
            ),
            "docs/python-migration.md": "[text-option guide](text-options.md)",
            "docs/rust-api.md": "[text-option guide](text-options.md)",
            "docs/rust-data-models.md": "[text-option guide](text-options.md)",
        }
        for relative, link in links.items():
            with self.subTest(document=relative):
                rendered = (REPO_ROOT / relative).read_text(encoding="utf-8")
                self.assertIn(link, rendered)

        changelog = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        with self.subTest(document="CHANGELOG.md"):
            self.assertRegex(
                changelog,
                r"(?im)^- \*\*Text options:\*\* .*text-option guide",
            )

    def test_every_pinned_word_and_textmap_option_has_its_exact_default(self) -> None:
        for option, default in {**WORD_OPTIONS, **TEXTMAP_OPTIONS}.items():
            with self.subTest(option=option):
                self.assertIn(f"| `{option}` | `{default}` |", self.guide)

        for method in (
            "`Page.extract_words`",
            "`Page.extract_text`",
            "`Page.extract_text_simple`",
            "`Page.extract_text_lines`",
            "`Page.search`",
            "`utils.extract_words`",
            "`utils.extract_text`",
            "`utils.extract_text_simple`",
            "`utils.chars_to_textmap`",
        ):
            with self.subTest(method=method):
                self.assertIn(method, self.guide)

        for option, default in (
            ("return_chars", "False"),
            ("strip", "True"),
            ("regex", "True"),
            ("case", "True"),
            ("main_group", "0"),
            ("return_groups", "True"),
        ):
            with self.subTest(method_option=option):
                self.assertIn(f"| `{option}` | `{default}` |", self.guide)

    def test_compatible_examples_exercise_every_option_family(self) -> None:
        for example in (
            "page.extract_words(",
            "x_tolerance=1",
            "y_tolerance=1",
            "x_tolerance_ratio=0.4",
            "y_tolerance_ratio=0.4",
            "keep_blank_chars=True",
            "use_text_flow=True",
            "vertical_ttb=False",
            "horizontal_ltr=False",
            'line_dir="btt"',
            'char_dir="rtl"',
            'line_dir_rotated="rtl"',
            'char_dir_rotated="btt"',
            'extra_attrs=["fontname", "size"]',
            "split_at_punctuation=True",
            'split_at_punctuation=",.;"',
            "expand_ligatures=False",
            "return_chars=True",
            "page.extract_text(",
            "layout=True",
            "layout_width=500",
            "layout_height=700",
            "layout_width_chars=70",
            "layout_height_chars=50",
            "layout_bbox=page.bbox",
            "x_density=8",
            "y_density=14",
            "x_shift=2",
            "y_shift=2",
            'char_dir_render="rtl"',
            'line_dir_render="btt"',
            "page.extract_text_simple(x_tolerance=1, y_tolerance=1)",
            "page.extract_text_lines(strip=False, return_chars=False",
            "page.search(",
            'pattern=r"(invoice)\\s+(\\d+)"',
            "regex=True",
            "case=False",
            "main_group=2",
            "return_groups=False",
            "utils.extract_text(chars, presorted=True)",
        ):
            with self.subTest(example=example):
                self.assertIn(example, self.guide)

        self.assertGreaterEqual(self.guide.count("```python"), 6)

    def test_option_interactions_and_method_scopes_are_unambiguous(self) -> None:
        for statement in (
            "`x_tolerance_ratio` replaces the fixed horizontal tolerance with the previous character's `size` multiplied by the ratio",
            "`y_tolerance_ratio` applies the corresponding dynamic rule on the vertical axis",
            "`vertical_ttb=False` and `horizontal_ltr=False` still affect ordering but emit deprecation warnings",
            "`line_dir` and `char_dir` must be orthogonal",
            "omitted rotated directions cross-default: `line_dir_rotated = char_dir` and `char_dir_rotated = line_dir`",
            "`split_at_punctuation=True` means Python's complete `string.punctuation`",
            "`extra_attrs` both separates words on attribute changes and copies those attributes into each word dictionary",
            "`layout_width` and `layout_width_chars` are mutually exclusive",
            "`layout_height` and `layout_height_chars` are mutually exclusive",
            "layout dimensions, density, bounding box, and shifts affect layout output only when `layout=True`",
            "`line_dir_render` and `char_dir_render` transform the rendered TextMap after grouping directions have been applied",
            "`extract_text_lines` defaults to `strip=True` and `return_chars=True`",
            "zero-width and all-whitespace search matches are discarded",
            "`main_group` selects the regex group whose text, characters, and bounding box become the primary match",
            "compiled regular-expression patterns are accepted",
            "`presorted` belongs to the public utility path, not `Page.extract_text`",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_current_surfaces_and_claim_limits_are_not_conflated(self) -> None:
        for statement in (
            "The current Python `Page.extract_text` accepts only `layout`",
            "The current Python `Page.extract_words` accepts only `x_tolerance` and `y_tolerance`",
            "The current Python `Page.search` accepts only `pattern`, `regex`, and `case`",
            "The current Python surface does not expose `extract_text_simple` or `extract_text_lines`",
            "The current WebAssembly surface exposes the same narrow text, word, and search controls as the Python adapter",
            "Rust `WordOptions` exposes fixed and ratio tolerances, blank handling, content-flow ordering, one `text_direction`, ligature expansion, and punctuation splitting",
            "Rust `TextOptions` defaults to `x_density=10` and `y_density=10`, not the pinned Python defaults",
            "Rust `ColumnMode`, `min_column_gap`, and `max_columns` are extensions, not Python-compatible text options",
            "In current Rust text and text-line extraction, only `y_tolerance` and `expand_ligatures` are forwarded into word grouping",
            "matching an option name or default does not establish matching output",
            "The pinned 111-case text option matrix records reference behavior; it is not candidate parity evidence",
            "The versioned scorecard labels Text, Words, and Search as observed evidence rather than workflow-level passes",
            "text-option documentation is not compatibility evidence",
            "does not approve a compatibility deviation",
            "TEXT-001 through TEXT-SEARCH-008 remain open",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

        for source in (
            "https://github.com/jsvine/pdfplumber/blob/v0.11.10/README.md",
            "https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/page.py",
            "https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/utils/text.py",
            "../compat/harness/option_matrix.py",
            "../crates/pdfplumber-core/src/words.rs",
            "../crates/pdfplumber-core/src/layout.rs",
            "../crates/pdfplumber-py/src/lib.rs",
            "../crates/pdfplumber-wasm/src/lib.rs",
            "compatibility/workflows-v0.3.0.md#text",
        ):
            with self.subTest(source=source):
                self.assertIn(source, self.guide)

        self.assertNotRegex(self.guide, re.compile(r"\b\d+(?:\.\d+)?%\b"))


if __name__ == "__main__":
    unittest.main()
