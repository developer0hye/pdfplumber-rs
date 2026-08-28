"""Contract tests for the pinned-upstream text/table option matrix (PARITY-011)."""

from __future__ import annotations

import json
import logging
import tempfile
import unittest
import warnings
from pathlib import Path
from unittest import mock

from compat.harness import lockfile, option_matrix, upstream
from scripts import generate_option_matrix


SNAPSHOT_PATH: Path = (
    upstream.REPO_ROOT
    / "compat"
    / "snapshots"
    / "pdfplumber-v0.11.10-option-matrix.json"
)

# Independent contract derived from the v0.11.10 README, public signatures,
# and TableSettings. Options are API-scoped because a shared keyword can take a
# different path through extract_words, extract_text, text lines, and search.
WORD_DEFAULTS: dict[str, object] = {
    "x_tolerance": 3,
    "y_tolerance": 3,
    "x_tolerance_ratio": None,
    "y_tolerance_ratio": None,
    "keep_blank_chars": False,
    "use_text_flow": False,
    "vertical_ttb": True,
    "horizontal_ltr": True,
    "line_dir": "ttb",
    "char_dir": "ltr",
    "line_dir_rotated": None,
    "char_dir_rotated": None,
    "extra_attrs": None,
    "split_at_punctuation": False,
    "expand_ligatures": True,
}

TEXTMAP_DEFAULTS: dict[str, object] = {
    "layout": False,
    "layout_width": 0,
    "layout_height": 0,
    "layout_width_chars": 0,
    "layout_height_chars": 0,
    "layout_bbox": (0, 0, 0, 0),
    "x_density": 7.25,
    "y_density": 13,
    "x_shift": 0,
    "y_shift": 0,
    "char_dir_render": None,
    "line_dir_render": None,
    "presorted": False,
}

# Page.extract_text exposes every WordExtractor/TextMap option except that
# presorted is intentionally exercised through the public utility surface.
PAGE_TEXT_DEFAULTS: dict[str, object] = {
    **WORD_DEFAULTS,
    **{key: value for key, value in TEXTMAP_DEFAULTS.items() if key != "presorted"},
}

TEXT_DEFAULTS: dict[str, object] = {
    **{f"extract_words.{key}": value for key, value in WORD_DEFAULTS.items()},
    "extract_words.return_chars": False,
    **{f"extract_text.{key}": value for key, value in PAGE_TEXT_DEFAULTS.items()},
    "extract_text_simple.x_tolerance": 3,
    "extract_text_simple.y_tolerance": 3,
    **{
        f"extract_text_lines.{key}": value
        for key, value in PAGE_TEXT_DEFAULTS.items()
    },
    "extract_text_lines.return_chars": True,
    "extract_text_lines.strip": True,
    **{f"search.{key}": value for key, value in PAGE_TEXT_DEFAULTS.items()},
    "search.regex": True,
    "search.case": True,
    "search.main_group": 0,
    "search.return_chars": True,
    "search.return_groups": True,
    "utils.extract_text.presorted": False,
}
EXPECTED_TEXT_OPTIONS: frozenset[str] = frozenset(TEXT_DEFAULTS)

TABLE_BASE_OPTIONS: frozenset[str] = frozenset(
    {
        "edge_min_length",
        "edge_min_length_prefilter",
        "explicit_horizontal_lines",
        "explicit_vertical_lines",
        "horizontal_strategy",
        "intersection_tolerance",
        "intersection_x_tolerance",
        "intersection_y_tolerance",
        "join_tolerance",
        "join_x_tolerance",
        "join_y_tolerance",
        "min_words_horizontal",
        "min_words_vertical",
        "snap_tolerance",
        "snap_x_tolerance",
        "snap_y_tolerance",
        "text_tolerance",
        "text_x_tolerance",
        "text_y_tolerance",
        "vertical_strategy",
    }
)

# TableSettings documents every text-extraction keyword under a text_ prefix,
# including the utility-level presorted keyword used by chars_to_textmap.
TEXT_OPTIONS_ACCEPTED_BY_EXTRACT_TEXT: frozenset[str] = frozenset(
    set(WORD_DEFAULTS) | set(TEXTMAP_DEFAULTS)
)
EXPECTED_TABLE_OPTIONS: frozenset[str] = TABLE_BASE_OPTIONS | frozenset(
    f"text_{option}" for option in TEXT_OPTIONS_ACCEPTED_BY_EXTRACT_TEXT
)

TABLE_DEFAULTS: dict[str, object] = {
    "vertical_strategy": "lines",
    "horizontal_strategy": "lines",
    "explicit_vertical_lines": None,
    "explicit_horizontal_lines": None,
    "snap_tolerance": 3,
    "snap_x_tolerance": 3,
    "snap_y_tolerance": 3,
    "join_tolerance": 3,
    "join_x_tolerance": 3,
    "join_y_tolerance": 3,
    "edge_min_length": 3,
    "edge_min_length_prefilter": 1,
    "min_words_vertical": 3,
    "min_words_horizontal": 1,
    "intersection_tolerance": 3,
    "intersection_x_tolerance": 3,
    "intersection_y_tolerance": 3,
    "text_tolerance": 3,
    "text_x_tolerance": 3,
    "text_y_tolerance": 3,
}
TABLE_DEFAULTS.update(
    {
        f"text_{option}": ({**WORD_DEFAULTS, **TEXTMAP_DEFAULTS})[option]
        for option in TEXT_OPTIONS_ACCEPTED_BY_EXTRACT_TEXT
    }
)


class OptionMatrixCatalogTests(unittest.TestCase):
    def test_every_documented_option_has_a_non_default_case(self) -> None:
        self.assertSetEqual(set(TEXT_DEFAULTS), set(EXPECTED_TEXT_OPTIONS))
        self.assertSetEqual(set(TABLE_DEFAULTS), set(EXPECTED_TABLE_OPTIONS))
        cases: tuple[option_matrix.Case, ...] = option_matrix.cases()
        identifiers: list[str] = [case.identifier for case in cases]
        self.assertEqual(len(identifiers), len(set(identifiers)))

        covered_text: set[str] = set()
        covered_table: set[str] = set()
        for case in cases:
            self.assertTrue(case.fixture.is_file(), case.identifier)
            self.assertGreaterEqual(case.page_number, 1, case.identifier)
            self.assertTrue(case.options, case.identifier)
            self.assertTrue(case.covers, case.identifier)
            defaults: dict[str, object] = (
                TEXT_DEFAULTS if case.domain == "text" else TABLE_DEFAULTS
            )
            for option in case.covers:
                keyword: str = option.rsplit(".", 1)[-1]
                self.assertIn(keyword, case.options, case.identifier)
                self.assertNotEqual(
                    case.options[keyword], defaults[option], case.identifier
                )
            if case.domain == "text":
                covered_text.update(case.covers)
            elif case.domain == "table":
                covered_table.update(case.covers)
            else:
                self.fail(f"{case.identifier}: unknown domain {case.domain!r}")

        self.assertSetEqual(covered_text, set(EXPECTED_TEXT_OPTIONS))
        self.assertSetEqual(covered_table, set(EXPECTED_TABLE_OPTIONS))


class OptionMatrixSnapshotTests(unittest.TestCase):
    def test_committed_snapshot_is_complete_and_traceable(self) -> None:
        self.assertTrue(
            SNAPSHOT_PATH.is_file(),
            f"missing option-matrix snapshot: {SNAPSHOT_PATH.relative_to(upstream.REPO_ROOT)}",
        )
        payload: dict[str, object] = json.loads(
            SNAPSHOT_PATH.read_text(encoding="utf-8")
        )
        target: upstream.Target = upstream.load_target()

        self.assertEqual(payload["schema_version"], 1)
        self.assertEqual(
            payload["target"],
            {
                "project": target.project,
                "version": target.version,
                "tag": target.tag,
                "commit": target.commit,
                "repository": target.repository,
            },
        )
        self.assertEqual(payload["lockfile_sha256"], lockfile.digest())
        self.assertEqual(
            payload["python_version"],
            upstream.load_environment().python_version,
        )

        records: list[dict[str, object]] = payload["cases"]  # type: ignore[assignment]
        outputs: dict[str, object] = payload["outputs"]  # type: ignore[assignment]
        catalog: tuple[option_matrix.Case, ...] = option_matrix.cases()
        self.assertEqual(
            [record["id"] for record in records],
            [case.identifier for case in catalog],
        )
        self.assertTrue(records)
        self.assertTrue(all(record["status"] == "ok" for record in records))
        for record in records:
            for required_key in (
                "arguments",
                "covers",
                "fixture_path",
                "fixture_sha256",
                "logs",
                "options",
                "page_number",
                "result",
                "warnings",
            ):
                self.assertIn(required_key, record, record["id"])
            fixture: Path = upstream.REPO_ROOT / str(record["fixture_path"])
            self.assertTrue(fixture.is_file(), record["id"])
            result: dict[str, str] = record["result"]  # type: ignore[assignment]
            self.assertEqual(set(result), {"$ref"}, record["id"])
            self.assertIn(result["$ref"], outputs, record["id"])
            self.assertEqual(
                record["fixture_sha256"],
                option_matrix.file_sha256(fixture),
                record["id"],
            )

    def test_candidate_mode_writes_an_isolated_machine_snapshot(self) -> None:
        snapshot = {
            "schema_version": 1,
            "target": {
                "project": "pdfplumber",
                "version": "0.11.10",
                "commit": "7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62",
            },
            "outputs": {},
            "cases": [],
        }
        candidate_package = object()
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "candidate-options.json"
            with (
                mock.patch(
                    "sys.argv",
                    [
                        "generate_option_matrix.py",
                        "--candidate-output",
                        str(output),
                    ],
                ),
                mock.patch.object(
                    generate_option_matrix.environment,
                    "verify_candidate",
                ) as verify_candidate,
                mock.patch.object(
                    generate_option_matrix.environment,
                    "verify_reference",
                ) as verify_reference,
                mock.patch.object(
                    generate_option_matrix.option_matrix,
                    "build",
                    return_value=snapshot,
                ),
                mock.patch("builtins.print"),
            ):
                status = generate_option_matrix.main(candidate_package)

            payload = json.loads(output.read_text(encoding="utf-8"))

        self.assertEqual(status, 0)
        self.assertEqual(payload, snapshot)
        verify_candidate.assert_called_once_with(
            candidate_package,
            expected_root=generate_option_matrix.environment.CANDIDATE_VENV,
        )
        verify_reference.assert_not_called()

    def test_candidate_error_records_keep_diagnostics_and_identity(self) -> None:
        class Page:
            def extract_words(self, **options: object) -> object:
                warnings.warn("candidate warning", UserWarning)
                logging.getLogger("pdfplumber").warning("candidate log")
                raise TypeError(
                    f"unexpected option {next(iter(options))}"
                )

        class Pdf:
            pages = [Page()]

            def __enter__(self) -> "Pdf":
                return self

            def __exit__(self, *args: object) -> None:
                return None

        class Package:
            @staticmethod
            def open(path: Path) -> Pdf:
                self.assertEqual(path, option_matrix.TEXT_FIXTURE)
                return Pdf()

        case = option_matrix.Case(
            identifier="text.extract_words.candidate_only",
            domain="text",
            api="extract_words",
            fixture=option_matrix.TEXT_FIXTURE,
            page_number=1,
            options={"candidate_only": True},
            covers=("extract_words.candidate_only",),
            arguments={},
        )

        record = option_matrix._run_case(Package(), case)

        self.assertEqual(record["status"], "error")
        self.assertEqual(record["options"], {"candidate_only": True})
        self.assertEqual(record["arguments"], {})
        self.assertEqual(
            record["warnings"],
            [{"category": "builtins.UserWarning", "message": "candidate warning"}],
        )
        self.assertEqual(
            record["logs"],
            [
                {
                    "level": "WARNING",
                    "logger": "pdfplumber",
                    "message": "candidate log",
                }
            ],
        )
        self.assertEqual(record["error"]["type"], "builtins.TypeError")


if __name__ == "__main__":
    unittest.main()
