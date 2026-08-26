"""Contracts for the privacy and local-processing statement (ADOPT-016)."""

from __future__ import annotations

import ast
import re
import unittest
from pathlib import Path

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]
PRIVACY_PATH = REPO_ROOT / "docs" / "privacy.md"
README_PATH = REPO_ROOT / "README.md"
FAQ_PATH = REPO_ROOT / "docs" / "faq.md"
CHANGELOG_PATH = REPO_ROOT / "CHANGELOG.md"
ROADMAP_PATH = REPO_ROOT / "ROADMAP.md"
REPAIR_PATH = (
    REPO_ROOT
    / "crates"
    / "pdfplumber-py"
    / "python"
    / "pdfplumber"
    / "repair.py"
)

EXPECTED_SECTIONS = [
    "Local extraction boundary",
    "Optional external executable: Python repair",
    "Boundaries outside the project runtime",
]

RUNTIME_DEPENDENCIES = {
    "crates/pdfplumber-core/Cargo.toml": {
        "regex",
        "serde",
        "unicode-bidi",
        "unicode-normalization",
    },
    "crates/pdfplumber-parse/Cargo.toml": {
        "encoding_rs",
        "lopdf",
        "pdfplumber-core",
        "thiserror",
        "tracing",
    },
    "crates/pdfplumber/Cargo.toml": {
        "pdfplumber-core",
        "pdfplumber-parse",
        "rayon",
    },
    "crates/pdfplumber-cli/Cargo.toml": {"clap", "pdfplumber", "serde_json"},
    "crates/pdfplumber-py/Cargo.toml": {"pdfplumber", "pyo3"},
    "crates/pdfplumber-wasm/Cargo.toml": {
        "pdfplumber",
        "serde-wasm-bindgen",
        "wasm-bindgen",
    },
}

PYTHON_RUNTIME_IMPORTS = {
    "__future__",
    "_native",
    "base64",
    "collections",
    "csv",
    "exceptions",
    "io",
    "pathlib",
    "shutil",
    "subprocess",
    "typing",
}

FORBIDDEN_RUST_RUNTIME_PRIMITIVES = {
    "std::net",
    "TcpStream",
    "UdpSocket",
    "std::process::Command",
    "tokio::process",
    "Command::new(",
    "web_sys",
    "XmlHttpRequest",
    "fetch(",
}


class PrivacyStatementContractTests(unittest.TestCase):
    def statement(self) -> str:
        self.assertTrue(PRIVACY_PATH.is_file(), "docs/privacy.md is missing")
        if not PRIVACY_PATH.is_file():
            return ""
        return PRIVACY_PATH.read_text(encoding="utf-8")

    def test_statement_is_maintained_and_linked_from_user_guidance(self) -> None:
        statement = self.statement()
        self.assertTrue(statement.startswith("# Privacy and local processing\n"))
        self.assertEqual(
            re.findall(r"^## ([^\n]+)$", statement, re.MULTILINE),
            EXPECTED_SECTIONS,
        )

        readme = README_PATH.read_text(encoding="utf-8")
        self.assertIn("[Privacy and local processing](docs/privacy.md)", readme)
        faq = FAQ_PATH.read_text(encoding="utf-8")
        self.assertIn("[privacy statement](privacy.md)", faq)
        changelog = CHANGELOG_PATH.read_text(encoding="utf-8")
        self.assertIn("privacy and local-processing statement", changelog)

        roadmap = ROADMAP_PATH.read_text(encoding="utf-8")
        self.assertNotIn("ADOPT-016", roadmap)
        self.assertIn("[`ADOPT-017`]", roadmap)

    def test_statement_defines_local_processing_without_blanket_promises(self) -> None:
        statement = self.statement()
        for fact in (
            "Rust, Python, Command-Line Interface, and WebAssembly",
            "does not upload documents",
            "document contents, extracted text, or document metadata",
            "does not collect usage telemetry, analytics, or crash reports",
            "host application",
            "WebAssembly wrapper receives bytes",
            "package managers",
            "documentation links",
        ):
            with self.subTest(fact=fact):
                self.assertIn(fact, statement)

        for overclaim in (
            "all data always stays on your device",
            "never uses the network",
            "Ghostscript cannot access the network",
        ):
            with self.subTest(overclaim=overclaim):
                self.assertNotIn(overclaim, statement)

    def test_statement_discloses_the_complete_ghostscript_boundary(self) -> None:
        statement = self.statement()
        for fact in (
            "`Pdf::open_with_repair`",
            "`--repair`",
            "in-process native repair",
            "`repair=False`",
            "`repair=True`",
            "`gs_path`",
            "`gs`",
            "`gswin32c`",
            "`gswin64c`",
            "file path as a child-process argument",
            "stream bytes through standard input",
            "`-sPDFPassword=...`",
            "local process inspection",
            "standard output",
            "standard error",
            "Ghostscript is outside",
        ):
            with self.subTest(fact=fact):
                self.assertIn(fact, statement)

    def test_documented_runtime_boundary_matches_source(self) -> None:
        for relative_path, expected in RUNTIME_DEPENDENCIES.items():
            with self.subTest(manifest=relative_path):
                manifest = tomllib.loads(
                    (REPO_ROOT / relative_path).read_text(encoding="utf-8")
                )
                self.assertEqual(set(manifest.get("dependencies", {})), expected)

        wasm = tomllib.loads(
            (REPO_ROOT / "crates/pdfplumber-wasm/Cargo.toml").read_text(
                encoding="utf-8"
            )
        )
        wasm_targets = wasm.get("target", {})
        self.assertEqual(len(wasm_targets), 1)
        target_dependencies = next(iter(wasm_targets.values())).get(
            "dependencies", {}
        )
        self.assertEqual(set(target_dependencies), {"getrandom"})

        python_imports: set[str] = set()
        python_root = REPO_ROOT / "crates/pdfplumber-py/python/pdfplumber"
        for path in python_root.rglob("*.py"):
            tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
            for node in ast.walk(tree):
                if isinstance(node, ast.Import):
                    python_imports.update(
                        name.name.split(".", 1)[0] for name in node.names
                    )
                elif isinstance(node, ast.ImportFrom) and node.module:
                    python_imports.add(node.module.split(".", 1)[0])
        self.assertEqual(python_imports, PYTHON_RUNTIME_IMPORTS)

        rust_runtime = "\n".join(
            path.read_text(encoding="utf-8")
            for path in (REPO_ROOT / "crates").glob("*/src/**/*.rs")
        )
        for primitive in FORBIDDEN_RUST_RUNTIME_PRIMITIVES:
            with self.subTest(rust_runtime_primitive=primitive):
                self.assertNotIn(primitive, rust_runtime)

        repair = REPAIR_PATH.read_text(encoding="utf-8")
        lookup_order = [
            repair.index(token)
            for token in (
                "gs_path\n",
                'shutil.which("gs")',
                'shutil.which("gswin32c")',
                'shutil.which("gswin64c")',
            )
        ]
        self.assertEqual(lookup_order, sorted(lookup_order))
        for source_fact in (
            'f"-sPDFPassword={password}"',
            'repair_args += ["-"]',
            "stdin=subprocess.PIPE if stdin else None",
            "stdout=subprocess.PIPE",
            "stderr=subprocess.PIPE",
            "subprocess.Popen(",
        ):
            with self.subTest(source_fact=source_fact):
                self.assertIn(source_fact, repair)

        for target in re.findall(r"\[[^\]]+\]\(([^)]+)\)", self.statement()):
            if "://" in target:
                continue
            relative = target.split("#", 1)[0]
            with self.subTest(local_link=target):
                self.assertTrue((PRIVACY_PATH.parent / relative).is_file())


if __name__ == "__main__":
    unittest.main()
