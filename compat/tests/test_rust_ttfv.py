"""Contracts for the clean-project Rust time-to-first-value path (DX-018)."""

from __future__ import annotations

import json
import subprocess
import sys
import unittest
from pathlib import Path

import tomllib

ROOT = Path(__file__).resolve().parents[2]
TOOL = ROOT / "scripts/measure_rust_ttfv.py"
GUIDE = ROOT / "docs/rust-ttfv.md"
RESULT = ROOT / "docs/measurements/rust-ttfv-workspace-2026-08-30.json"
README = (ROOT / "README.md").read_text(encoding="utf-8")
WORKFLOW = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
SUPPORT_SOURCE = (ROOT / "support-matrix.toml").read_text(encoding="utf-8")
CHANGELOG = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
ROADMAP = (ROOT / "ROADMAP.md").read_text(encoding="utf-8")
PRD = (ROOT / "PRD.md").read_text(encoding="utf-8")
REFERENCE_INDEX = (ROOT / "references/INDEX.md").read_text(encoding="utf-8")
RELEASE_VERSION = tomllib.loads(
    (ROOT / "Cargo.toml").read_text(encoding="utf-8")
)["workspace"]["package"]["version"]


class RustTtfvContractTests(unittest.TestCase):
    def tool_source(self) -> str:
        self.assertTrue(TOOL.is_file(), f"missing measurement tool: {TOOL}")
        if not TOOL.is_file():
            return ""
        return TOOL.read_text(encoding="utf-8")

    def result(self) -> dict[str, object]:
        self.assertTrue(RESULT.is_file(), f"missing measurement result: {RESULT}")
        if not RESULT.is_file():
            return {}
        return json.loads(RESULT.read_text(encoding="utf-8"))

    def test_protocol_starts_from_a_new_project_with_cold_cargo_state(self) -> None:
        source = self.tool_source()

        for fragment in (
            "cargo new",
            "--bin",
            "--vcs",
            "none",
            "CARGO_HOME",
            "TemporaryDirectory",
            "Cargo.lock",
            "cargo run",
            "surface_snippets",
            "PRIMARY_RUST_OUTPUT_MARKER",
        ):
            with self.subTest(fragment=fragment):
                self.assertIn(fragment, source)
        self.assertNotIn("cargo fetch", source)
        self.assertNotIn("cargo build", source)

    def test_versioned_result_includes_every_ttfv_component_under_five_minutes(
        self,
    ) -> None:
        result = self.result()
        if not result:
            return

        self.assertEqual(result["schema_version"], 1)
        self.assertEqual(result["result"], "pass")
        self.assertEqual(result["threshold_seconds"], 300)
        self.assertLessEqual(result["total_seconds"], result["threshold_seconds"])
        self.assertEqual(
            set(result["coverage"]),
            {"installation", "code_copy", "execution", "interpretation"},
        )
        self.assertTrue(all(result["coverage"].values()))
        self.assertEqual(
            set(result["phases"]),
            {
                "project_creation",
                "dependency_declaration",
                "code_copy",
                "resolve_build_execute",
                "interpretation",
            },
        )

    def test_result_is_bound_to_current_source_docs_fixture_and_environment(self) -> None:
        result = self.result()
        if not result:
            return

        self.assertEqual(result["source"]["kind"], "workspace candidate")
        self.assertEqual(result["source"]["crate"], "pdfplumber")
        self.assertEqual(result["source"]["resolved_version"], RELEASE_VERSION)
        self.assertRegex(result["source"]["tree_sha256"], r"^[0-9a-f]{64}$")
        self.assertEqual(result["isolation"]["project"], "new cargo project")
        self.assertEqual(result["isolation"]["cargo_home"], "empty temporary directory")
        self.assertEqual(result["isolation"]["target"], "new project-local directory")
        for digest in ("installation_sha256", "quick_start_sha256", "fixture_sha256"):
            with self.subTest(digest=digest):
                self.assertRegex(result["inputs"][digest], r"^[0-9a-f]{64}$")
        self.assertRegex(result["inputs"]["cargo_lock_sha256"], r"^[0-9a-f]{64}$")
        for key in ("os", "architecture", "rustc", "cargo"):
            with self.subTest(key=key):
                self.assertTrue(result["environment"][key])
        self.assertEqual(result["registry_trial"]["result"], "compile failure")
        self.assertIn("Pdf::open_path", result["registry_trial"]["reason"])
        self.assertIn("DIST-001", result["registry_trial"]["disposition"])
        self.assertIn("DIST-007", result["registry_trial"]["disposition"])

    def test_methodology_defines_clock_boundary_prerequisite_and_limitations(self) -> None:
        self.assertTrue(GUIDE.is_file(), f"missing methodology: {GUIDE}")
        if not GUIDE.is_file():
            return
        guide = GUIDE.read_text(encoding="utf-8")
        normalized = " ".join(guide.split())

        self.assertIn("## Clock boundary", guide)
        self.assertIn("## Reproduce", guide)
        self.assertIn("rust-ttfv-workspace-2026-08-30.json", guide)
        self.assertRegex(normalized, r"(?i)starts before.*cargo new")
        self.assertRegex(normalized, r"(?i)stops after.*interpret")
        self.assertRegex(normalized, r"(?i)stable Rust.*prerequisite")
        self.assertRegex(normalized, r"(?i)empty.*CARGO_HOME")
        self.assertRegex(normalized, r"(?i)not.*human.*reading|does not.*human.*reading")
        self.assertRegex(normalized, r"(?i)network.*var")
        self.assertRegex(normalized, r"(?i)remeasure.*release")
        self.assertRegex(normalized, r"(?i)published.*0\.3\.0.*open_path")
        self.assertIn("DIST-001", guide)
        self.assertIn("DIST-007", guide)

    def test_primary_path_precedes_optional_concepts_and_names_the_only_input(self) -> None:
        installation = README.index("## Installation")
        quick_start = README.index("## Quick Start", installation)
        rust_fence = README.index("```rust", quick_start)
        primary_end = README.index("```", rust_fence + len("```rust"))
        activation_path = README[installation:primary_end]

        self.assertNotIn("Feature Flags", activation_path)
        for concept in ("pdfplumber-core", "pdfplumber-parse", "rayon", "serde", "async"):
            with self.subTest(concept=concept):
                self.assertNotIn(concept, activation_path.lower())
        self.assertRegex(activation_path, r"(?i)searchable PDF.*document\.pdf")
        self.assertRegex(activation_path, r"(?i)only.*pdfplumber.*dependenc")
        self.assertIn("docs/rust-ttfv.md", README)

    def test_checker_and_public_evidence_are_wired_into_ci(self) -> None:
        result_path = RESULT.relative_to(ROOT).as_posix()
        self.assertIn(
            f"python scripts/measure_rust_ttfv.py --check {result_path}",
            WORKFLOW,
        )
        self.assertIn("docs/rust-ttfv.md", SUPPORT_SOURCE)
        self.assertIn(
            "docs/measurements/rust-ttfv-workspace-2026-08-30.json",
            SUPPORT_SOURCE,
        )
        self.assertIn("compat/tests/test_rust_ttfv.py", SUPPORT_SOURCE)
        self.assertRegex(CHANGELOG, r"(?is)Rust.*time to first value.*five minutes")
        self.assertIn("rust-ttfv.md", REFERENCE_INDEX)

        completed = subprocess.run(
            [sys.executable, str(TOOL), "--check", str(RESULT)],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_roadmap_and_prd_advance_after_versioned_evidence(self) -> None:
        self.assertNotIn("### Measure and reduce Rust time to first value", ROADMAP)
        self.assertNotIn("DIST-001", ROADMAP)
        self.assertIn("SCORE-010", ROADMAP)
        self.assertIn("- [x] **DX-018**", PRD)
        self.assertRegex(
            PRD,
            r"(?m)^\| `DX-018` \| 2026-08-28 \| Codex \| PR #\d+ \|",
        )


if __name__ == "__main__":
    unittest.main()
