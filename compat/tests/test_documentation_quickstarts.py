from __future__ import annotations

import re
import subprocess
import sys
import unittest
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "scripts/check_doc_quickstarts.py"


@dataclass(frozen=True)
class Surface:
    document: str
    installation_language: str
    installation: str
    quick_start_languages: tuple[str, ...]


SURFACES = {
    "rust": Surface(
        document="README.md",
        installation_language="toml",
        installation='[dependencies]\npdfplumber = "0.3"',
        quick_start_languages=("rust", "rust", "rust", "rust", "rust"),
    ),
    "python": Surface(
        document="crates/pdfplumber-py/README.md",
        installation_language="bash",
        installation="pip install pdfplumber-rs",
        quick_start_languages=("python",),
    ),
    "cli": Surface(
        document="crates/pdfplumber-cli/README.md",
        installation_language="bash",
        installation="cargo install pdfplumber-cli",
        quick_start_languages=("bash",),
    ),
    "wasm-node": Surface(
        document="crates/pdfplumber-wasm/README.md",
        installation_language="bash",
        installation="npm install pdfplumber-wasm",
        quick_start_languages=("javascript",),
    ),
}


FENCE = re.compile(r"^```([^\n]*)\n(.*?)^```\s*$", re.MULTILINE | re.DOTALL)


def section(text: str, heading: str) -> str:
    match = re.search(rf"^## {re.escape(heading)}[^\n]*$", text, re.MULTILINE)
    if match is None:
        raise AssertionError(f"missing level-two {heading!r} section")
    next_heading = re.search(r"^## ", text[match.end() :], re.MULTILINE)
    end = len(text) if next_heading is None else match.end() + next_heading.start()
    return text[match.end() : end]


def executable_fences(text: str) -> list[tuple[str, str]]:
    fences: list[tuple[str, str]] = []
    for match in FENCE.finditer(text):
        language = match.group(1).split(",", 1)[0].strip()
        if language in {"bash", "javascript", "python", "rust", "toml"}:
            fences.append((language, match.group(2).strip()))
    return fences


class DocumentationQuickStartContractTests(unittest.TestCase):
    def test_every_surface_has_exact_installation_and_quick_start_snippets(
        self,
    ) -> None:
        for name, surface in SURFACES.items():
            with self.subTest(surface=name):
                text = (ROOT / surface.document).read_text(encoding="utf-8")
                installation = executable_fences(section(text, "Installation"))
                self.assertEqual(
                    [(surface.installation_language, surface.installation)],
                    installation,
                )

                quick_starts = executable_fences(section(text, "Quick Start"))
                self.assertEqual(
                    surface.quick_start_languages,
                    tuple(language for language, _ in quick_starts),
                )

    def test_checker_and_ci_cover_every_surface(self) -> None:
        self.assertTrue(CHECKER.is_file(), f"missing checker: {CHECKER}")
        workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        for command in (
            "python scripts/check_doc_quickstarts.py --check",
            "python scripts/check_doc_quickstarts.py --rust --cli",
            "python scripts/check_doc_quickstarts.py --python --wheel dist/*.whl",
            "python scripts/check_doc_quickstarts.py --wasm-node --npm-package crates/pdfplumber-wasm/pkg-node",
        ):
            with self.subTest(command=command):
                self.assertIn(command, workflow)

    def test_static_checker_accepts_the_rendered_documentation(self) -> None:
        if not CHECKER.is_file():
            self.fail(f"missing checker: {CHECKER}")
        completed = subprocess.run(
            [sys.executable, str(CHECKER), "--check"],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(
            0,
            completed.returncode,
            f"{completed.stdout}\n{completed.stderr}",
        )


if __name__ == "__main__":
    unittest.main()
