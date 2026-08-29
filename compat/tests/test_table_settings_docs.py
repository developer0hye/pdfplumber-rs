"""Contracts for the complete table-setting guide (DOC-009)."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GUIDE_PATH = REPO_ROOT / "docs" / "table-settings.md"

TABLE_OPTIONS = {
    "vertical_strategy": '"lines"',
    "horizontal_strategy": '"lines"',
    "explicit_vertical_lines": "None",
    "explicit_horizontal_lines": "None",
    "snap_tolerance": "3",
    "snap_x_tolerance": "3",
    "snap_y_tolerance": "3",
    "join_tolerance": "3",
    "join_x_tolerance": "3",
    "join_y_tolerance": "3",
    "edge_min_length": "3",
    "edge_min_length_prefilter": "1",
    "min_words_vertical": "3",
    "min_words_horizontal": "1",
    "intersection_tolerance": "3",
    "intersection_x_tolerance": "3",
    "intersection_y_tolerance": "3",
    "text_tolerance": "3",
    "text_x_tolerance": "3",
    "text_y_tolerance": "3",
}

FORWARDED_TEXT_OPTIONS = {
    "text_x_tolerance": "3",
    "text_y_tolerance": "3",
    "text_x_tolerance_ratio": "None",
    "text_y_tolerance_ratio": "None",
    "text_keep_blank_chars": "False",
    "text_use_text_flow": "False",
    "text_vertical_ttb": "True",
    "text_horizontal_ltr": "True",
    "text_line_dir": '"ttb"',
    "text_char_dir": '"ltr"',
    "text_line_dir_rotated": "None",
    "text_char_dir_rotated": "None",
    "text_extra_attrs": "None",
    "text_split_at_punctuation": "False",
    "text_expand_ligatures": "True",
    "text_layout": "False",
    "text_layout_width": "0",
    "text_layout_height": "0",
    "text_layout_width_chars": "0",
    "text_layout_height_chars": "0",
    "text_layout_bbox": "(0, 0, 0, 0)",
    "text_x_density": "7.25",
    "text_y_density": "13",
    "text_x_shift": "0",
    "text_y_shift": "0",
    "text_char_dir_render": "None",
    "text_line_dir_render": "None",
    "text_presorted": "False",
}


def compact(text: str) -> str:
    return " ".join(text.split())


class TableSettingsDocumentationContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.guide = (
            GUIDE_PATH.read_text(encoding="utf-8") if GUIDE_PATH.is_file() else ""
        )
        cls.compact_guide = compact(cls.guide)

    def test_canonical_guide_is_linked_from_every_table_entry_point(self) -> None:
        with self.subTest(artifact="canonical guide"):
            self.assertTrue(GUIDE_PATH.is_file(), f"missing guide: {GUIDE_PATH}")

        links = {
            "README.md": "[table-setting guide](docs/table-settings.md)",
            "crates/pdfplumber-py/README.md": (
                "[table-setting guide](../../docs/table-settings.md)"
            ),
            "crates/pdfplumber-wasm/README.md": (
                "[table-setting guide](../../docs/table-settings.md)"
            ),
            "docs/faq.md": "[table-setting guide](table-settings.md)",
            "docs/pre-parity-python-migration.md": (
                "[table-setting guide](table-settings.md)"
            ),
            "docs/python-migration.md": "[table-setting guide](table-settings.md)",
            "docs/rust-api.md": "[table-setting guide](table-settings.md)",
            "docs/rust-data-models.md": "[table-setting guide](table-settings.md)",
        }
        for relative, link in links.items():
            with self.subTest(document=relative):
                rendered = (REPO_ROOT / relative).read_text(encoding="utf-8")
                self.assertIn(link, rendered)

        changelog = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        with self.subTest(document="CHANGELOG.md"):
            self.assertRegex(
                changelog,
                r"(?im)^- \*\*Table settings:\*\* .*table-setting guide",
            )

    def test_every_pinned_setting_has_its_exact_effective_default(self) -> None:
        self.assertEqual(
            len(set(TABLE_OPTIONS) | set(FORWARDED_TEXT_OPTIONS)),
            46,
        )
        for option, default in {**TABLE_OPTIONS, **FORWARDED_TEXT_OPTIONS}.items():
            with self.subTest(option=option):
                self.assertIn(f"| `{option}` | `{default}` |", self.guide)

        for raw_default in (
            "`snap_x_tolerance=UNSET`",
            "`snap_y_tolerance=UNSET`",
            "`join_x_tolerance=UNSET`",
            "`join_y_tolerance=UNSET`",
            "`intersection_x_tolerance=UNSET`",
            "`intersection_y_tolerance=UNSET`",
        ):
            with self.subTest(raw_default=raw_default):
                self.assertIn(raw_default, self.guide)

    def test_compatible_examples_cover_every_setting_family_and_method(self) -> None:
        for example in (
            "TableSettings.resolve(None)",
            "TableSettings(",
            "text_settings={",
            "TableSettings.resolve(settings_instance) is settings_instance",
            "page.find_tables(table_settings)",
            "page.find_table(table_settings)",
            "page.extract_tables(table_settings)",
            "page.extract_table(table_settings)",
            "page.debug_tablefinder(table_settings)",
            "im.debug_tablefinder(table_settings)",
            '"vertical_strategy": "lines"',
            '"horizontal_strategy": "lines_strict"',
            '"vertical_strategy": "text"',
            '"horizontal_strategy": "text"',
            '"vertical_strategy": "explicit"',
            '"horizontal_strategy": "explicit"',
            '"explicit_vertical_lines": [x0, x1]',
            '"explicit_horizontal_lines": [top, bottom]',
            '"explicit_vertical_lines": [page.rects[0]]',
            '"explicit_horizontal_lines": [page.rects[0]]',
            '"snap_tolerance": 1',
            '"snap_x_tolerance": 2',
            '"snap_y_tolerance": 2',
            '"join_tolerance": 1',
            '"join_x_tolerance": 2',
            '"join_y_tolerance": 2',
            '"edge_min_length": 1',
            '"edge_min_length_prefilter": 0.5',
            '"min_words_vertical": 2',
            '"min_words_horizontal": 2',
            '"intersection_tolerance": 1',
            '"intersection_x_tolerance": 2',
            '"intersection_y_tolerance": 2',
            '"text_tolerance": 1',
            '"text_x_tolerance": 1',
            '"text_y_tolerance": 1',
            '"text_x_tolerance_ratio": 0.4',
            '"text_y_tolerance_ratio": 0.4',
            '"text_keep_blank_chars": True',
            '"text_use_text_flow": True',
            '"text_vertical_ttb": False',
            '"text_horizontal_ltr": False',
            '"text_line_dir": "btt"',
            '"text_char_dir": "rtl"',
            '"text_line_dir_rotated": "rtl"',
            '"text_char_dir_rotated": "btt"',
            '"text_extra_attrs": ["fontname"]',
            '"text_split_at_punctuation": True',
            '"text_expand_ligatures": False',
            '"text_layout": True',
            '"text_layout_width": 500',
            '"text_layout_height": 700',
            '"text_layout_width_chars": 70',
            '"text_layout_height_chars": 50',
            '"text_layout_bbox": page.bbox',
            '"text_x_density": 8',
            '"text_y_density": 14',
            '"text_x_shift": 2',
            '"text_y_shift": 2',
            '"text_char_dir_render": "rtl"',
            '"text_line_dir_render": "btt"',
            '"text_presorted": True',
        ):
            with self.subTest(example=example):
                self.assertIn(example, self.guide)

        self.assertGreaterEqual(self.guide.count("```python"), 9)

    def test_pipeline_interactions_and_validation_are_unambiguous(self) -> None:
        for statement in (
            "each axis selects edges independently; explicit lines are then added; edges are snapped, joined, and filtered by final length; intersections become cells; contiguous cells become tables; and text is extracted per cell",
            "`lines` includes line objects and rectangle edges, while `lines_strict` keeps only line-object edges",
            "`text` synthesizes edges from aligned words",
            "`explicit` suppresses detected edges only on that axis",
            "numeric vertical coordinates span the page height and numeric horizontal coordinates span the page width",
            "object inputs are expanded with `obj_to_edges`, then only edges with the requested orientation are retained",
            "an explicit strategy requires at least two entries for its axis",
            "explicit lines augment `lines`, `lines_strict`, and `text`; they are not limited to the `explicit` strategy",
            "`snap_x_tolerance` groups vertical edges by x-position, while `snap_y_tolerance` groups horizontal edges by top-position",
            "`join_x_tolerance` closes horizontal endpoint gaps, while `join_y_tolerance` closes vertical endpoint gaps",
            "an omitted axis tolerance inherits its general tolerance, but an explicit zero remains zero",
            "`edge_min_length_prefilter` acts before strategy edges are merged and `edge_min_length` acts after snapping and joining",
            "`min_words_vertical` applies only to vertical `text` discovery and `min_words_horizontal` only to horizontal `text` discovery",
            "`intersection_x_tolerance` and `intersection_y_tolerance` decide whether orthogonal edges meet closely enough to form cell vertices",
            "dictionary resolution strips the `text_` prefix and stores the remainder in `TableSettings.text_settings`",
            "`text_tolerance` is a legacy shorthand that fills missing text x/y tolerances and is then removed",
            "when either strategy is `text`, the complete forwarded dictionary is first passed to `Page.extract_words`",
            "TextMap-only keys combined with a `text` strategy therefore raise the pinned `WordExtractor` unexpected-keyword `TypeError`",
            "the same forwarded dictionary is later passed to `Table.extract` for every discovered cell",
            "when the `layout` key is present, `Table.extract` replaces layout width, height, and bounding box with each cell's geometry",
            "active character-count layout dimensions conflict with the point dimensions injected by `Table.extract`",
            "non-negative validation covers edge tolerances and thresholds but does not validate forwarded `text_*` values",
            "`find_table` chooses the most cells, then the topmost and leftmost table",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_current_surfaces_extensions_and_claim_limits_are_separate(self) -> None:
        for statement in (
            "The current Python `Page.find_tables` and `Page.extract_tables` accept no settings argument",
            "The current Python adapter does not expose `TableSettings`, `find_table`, `extract_table`, or `debug_tablefinder`",
            "The current WebAssembly `findTables()` and `extractTables()` methods always use defaults and accept no settings",
            "The current Command-Line Interface exposes only `lattice`/`stream`, `snap_tolerance`, `join_tolerance`, and `text_tolerance`",
            "the Command-Line Interface copies its general snap/join values into both axis fields",
            "the current core pipeline does not consult the three Rust text-tolerance fields",
            "Rust `TableSettings.strategy` is a fallback for optional per-axis strategies",
            "Rust `Strategy::Lattice`, `LatticeStrict`, `Stream`, and `Explicit` correspond by intent to the four pinned strategy names",
            "Rust explicit inputs are numeric coordinates grouped under `ExplicitLines`; upstream object descriptors are not accepted by that type",
            "Rust general snap/join fields do not dynamically populate axis fields after construction",
            "Rust table discovery and cell extraction use `WordOptions::default()` instead of forwarding the pinned `text_*` dictionary",
            "Rust does not reproduce `TableSettings.resolve` or its pinned validation and error behavior",
            "Rust `extract_table` breaks an equal-cell-count tie by area, not by pinned top/left order",
            "`min_accuracy` is a Rust-only post-detection filter",
            "`duplicate_merged_content` is a Rust-only cell-content normalization",
            "matching a setting name or default does not establish matching output",
            "The pinned 50-case table option matrix records reference behavior; it is not candidate parity evidence",
            "table-setting documentation is not compatibility evidence",
            "does not approve a compatibility deviation",
            "TABLE-001 through TABLE-020 remain open",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

        for source in (
            "https://github.com/jsvine/pdfplumber/blob/v0.11.10/README.md",
            "https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/table.py",
            "https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/page.py",
            "text-options.md",
            "../compat/harness/option_matrix.py",
            "../crates/pdfplumber-core/src/table.rs",
            "../crates/pdfplumber/src/page.rs",
            "../crates/pdfplumber-py/src/lib.rs",
            "../crates/pdfplumber-cli/src/cli.rs",
            "../crates/pdfplumber-wasm/src/lib.rs",
            "compatibility/workflows-v0.3.0.md#tables",
        ):
            with self.subTest(source=source):
                self.assertIn(source, self.guide)

        self.assertNotRegex(self.guide, re.compile(r"\b\d+(?:\.\d+)?%\b"))


if __name__ == "__main__":
    unittest.main()
