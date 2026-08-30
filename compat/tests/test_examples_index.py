"""Outcome-oriented maintained-example index contracts (ECOSYS-001)."""

from __future__ import annotations

import re
import tomllib
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
INDEX_PATH = REPO_ROOT / "docs" / "examples.md"
SUPPORT_SOURCE = REPO_ROOT / "support-matrix.toml"
EXAMPLE_SOURCE_SUFFIXES = {".html", ".js", ".py", ".rs", ".ts"}
OUTCOME_SECTIONS = (
    "Extract content",
    "Inspect document details",
    "Handle protected or invalid input",
    "Produce repeatable automation output",
    "Explore local browser extraction",
)
ROW_PATTERN = re.compile(
    r"^\| \[`(?P<label>[^`]+)`\]\((?P<link>\.\./crates/[^)]+)\) "
    r"\| (?P<surface>Rust|WebAssembly) \| `(?P<maturity>[^`]+)` \| "
    r"(?P<command>.+) \|$",
    re.MULTILINE,
)


def maintained_example_sources() -> tuple[str, ...]:
    sources = (
        path.relative_to(REPO_ROOT).as_posix()
        for examples_dir in (REPO_ROOT / "crates").glob("*/examples")
        for path in examples_dir.rglob("*")
        if path.is_file() and path.suffix in EXAMPLE_SOURCE_SUFFIXES
    )
    return tuple(sorted(sources))


class ExamplesIndexContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.index = INDEX_PATH.read_text(encoding="utf-8") if INDEX_PATH.is_file() else ""
        cls.rows = tuple(ROW_PATTERN.finditer(cls.index))
        support = tomllib.loads(SUPPORT_SOURCE.read_text(encoding="utf-8"))
        cls.maturity_by_surface = {
            surface["name"]: surface["maturity"] for surface in support["surfaces"]
        }

    def test_index_is_discoverable_and_organized_by_user_outcome(self) -> None:
        self.assertTrue(INDEX_PATH.is_file(), f"missing {INDEX_PATH.relative_to(REPO_ROOT)}")

        readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
        self.assertIn("[examples by outcome](docs/examples.md)", readme)

        section_positions = []
        for section in OUTCOME_SECTIONS:
            with self.subTest(section=section):
                heading = f"## {section}"
                self.assertEqual(self.index.count(heading), 1)
                section_positions.append(self.index.index(heading))
        self.assertEqual(section_positions, sorted(section_positions))
        self.assertNotRegex(self.index, r"(?m)^## (?:Rust|WebAssembly|Crates?|Modules?)$")

    def test_every_maintained_example_is_indexed_exactly_once(self) -> None:
        indexed_paths = tuple(
            (INDEX_PATH.parent / match.group("link"))
            .resolve()
            .relative_to(REPO_ROOT)
            .as_posix()
            for match in self.rows
        )
        expected_paths = maintained_example_sources()

        self.assertEqual(len(indexed_paths), len(set(indexed_paths)))
        self.assertEqual(sorted(indexed_paths), list(expected_paths))
        self.assertGreaterEqual(len(expected_paths), 11)

    def test_each_entry_uses_the_checked_surface_maturity(self) -> None:
        self.assertEqual(
            set(self.maturity_by_surface),
            {"Rust", "Python", "Command-Line Interface", "WebAssembly"},
        )
        for match in self.rows:
            surface = match.group("surface")
            with self.subTest(example=match.group("label"), surface=surface):
                self.assertEqual(
                    match.group("maturity"), self.maturity_by_surface[surface]
                )

        for statement in (
            "Maturity labels come from [`support-matrix.toml`](../support-matrix.toml)",
            "An example does not raise a surface's maturity",
            "Rust examples are compiled in Continuous Integration",
            "WebAssembly browser demo is experimental",
            "does not complete `ECOSYS-006`",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.index)

    def test_commands_match_the_example_entry_points(self) -> None:
        for match in self.rows:
            source_path = (
                (INDEX_PATH.parent / match.group("link"))
                .resolve()
                .relative_to(REPO_ROOT)
            )
            command = match.group("command")
            with self.subTest(example=source_path.as_posix()):
                if source_path.suffix == ".rs":
                    self.assertIn(
                        f"cargo run -p pdfplumber --example {source_path.stem}",
                        command,
                    )
                else:
                    self.assertEqual(
                        source_path.as_posix(),
                        "crates/pdfplumber-wasm/examples/browser-demo.html",
                    )
                    self.assertIn("wasm-pack build --target web", command)

        command_by_label = {
            match.group("label"): match.group("command") for match in self.rows
        }
        self.assertIn("serialize_words.rs", command_by_label)
        self.assertIn("parallel_batch.rs", command_by_label)
        self.assertIn("--features serde", command_by_label["serialize_words.rs"])
        self.assertIn("--features parallel", command_by_label["parallel_batch.rs"])


if __name__ == "__main__":
    unittest.main()
